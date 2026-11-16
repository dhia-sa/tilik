//! Tool terdefinisi untuk "Tanya Data". AI hanya memilih `{ tool, params }` (bukan menulis SQL);
//! Rust memvalidasi parameter lalu menjalankan query terparameter dari `queries.rs`.

use crate::aggregates::{fmt_pct, fmt_rp};
use crate::analytics::anomaly::AnomalyParams;
use crate::analytics::period::{iso_week_label, month_start, parse_date, parse_period, previous_period, week_start};
use crate::analytics::scorecard::Weights;
use crate::error::AppResult;
use crate::queries::{self, GroupBy, Scope};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;

pub const TOOL_NAMES: [&str; 8] = ["spend_by", "commodity_price", "vendor_ranking", "price_change", "top_items", "alerts", "kpi_summary", "none"];

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ToolParams {
    pub group_by: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub category: Option<String>,
    pub vendor: Option<String>,
    pub item: Option<String>,
    pub commodity: Option<String>,
    pub weeks: Option<i64>,
    pub metric: Option<String>,
    pub order: Option<String>,
    pub limit: Option<i64>,
    pub period_a: Option<String>,
    pub period_b: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoice {
    pub tool: String,
    pub params: ToolParams,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SummaryOut {
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExplainOut {
    pub narrative: String,
}

fn nullable(t: &str) -> Value {
    json!({ "type": [t, "null"] })
}

/// Schema JSON untuk pemilihan tool. Semua field wajib ada (boleh null) dan
/// `additionalProperties: false` supaya kompatibel dengan Claude CLI dan Codex CLI.
pub fn select_schema() -> Value {
    let fields = [
        ("group_by", "string"), ("date_from", "string"), ("date_to", "string"), ("category", "string"), ("vendor", "string"),
        ("item", "string"), ("commodity", "string"), ("weeks", "integer"), ("metric", "string"), ("order", "string"),
        ("limit", "integer"), ("period_a", "string"), ("period_b", "string"), ("kind", "string"),
    ];
    let mut props = serde_json::Map::new();
    for (k, t) in fields {
        props.insert(k.to_string(), nullable(t));
    }
    json!({
        "type": "object",
        "properties": {
            "tool": { "type": "string", "enum": TOOL_NAMES },
            "params": {
                "type": "object",
                "properties": props,
                "required": fields.iter().map(|f| f.0).collect::<Vec<_>>(),
                "additionalProperties": false
            },
            "reason": { "type": "string" }
        },
        "required": ["tool", "params", "reason"],
        "additionalProperties": false
    })
}

pub fn summary_schema() -> Value {
    json!({ "type": "object", "properties": { "summary": { "type": "string" } }, "required": ["summary"], "additionalProperties": false })
}

pub fn explain_schema() -> Value {
    json!({ "type": "object", "properties": { "narrative": { "type": "string" } }, "required": ["narrative"], "additionalProperties": false })
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub reference: NaiveDate,
    pub default_from: NaiveDate,
    pub default_to: NaiveDate,
    pub weights: Weights,
    pub anomaly: AnomalyParams,
    pub categories: Vec<String>,
    pub commodities: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolResult {
    pub tool: String,
    pub params: Value,
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub note: Option<String>,
}

pub fn select_prompt(question: &str, ctx: &ToolContext, feedback: Option<&str>) -> String {
    let ws = week_start(ctx.reference);
    let ms = month_start(ctx.reference);
    let fb = feedback
        .map(|f| format!("\nPERCOBAAN SEBELUMNYA DITOLAK: {f}\nPerbaiki pilihan tool/parameter.\n"))
        .unwrap_or_default();
    format!(
        r#"Kamu adalah asisten data procurement aplikasi SpendView (data dummy PT Agrinesia Raya).
Tugasmu HANYA memilih SATU tool beserta parameternya untuk menjawab pertanyaan user. Jangan menulis SQL, jangan menjawab pertanyaannya.

Tanggal acuan (hari ini) = {today}. "Minggu ini" = {week} ({ws} s/d {today}). "Bulan ini" = {ms} s/d {today}. Data tersedia s/d {today}.
Kuartal = 3 bulan kalender (Q1 Jan-Mar, Q2 Apr-Jun, Q3 Jul-Sep, Q4 Okt-Des). "Tahun ini" = {year}-01-01 s/d {today}.

Kategori: {cats}
Komoditas: {coms}

Tool yang tersedia (parameter yang tidak dipakai isi null):
1. spend_by: group_by (category|vendor|item|month), date_from, date_to, category?, vendor?, limit? -> total spend per kelompok. Contoh: "Kategori apa yang paling besar kuartal ini?"
2. commodity_price: commodity, weeks (1-52) -> harga rata-rata tertimbang mingguan. Contoh: "Harga gula 8 minggu terakhir gimana?"
3. vendor_ranking: metric (score|on_time|late|quality|price), order (asc|desc), limit, category?, commodity?, date_from, date_to -> peringkat vendor. "paling sering telat" = metric late, order desc. Contoh: "Vendor gula mana yang paling sering telat bulan ini?"
4. price_change: item ATAU commodity, period_a, period_b (format YYYY-MM atau YYYY-Www; period_a = periode lama, period_b = periode baru) -> perubahan harga %. Contoh: "Tepung naik berapa persen dibanding bulan lalu?"
5. top_items: category, date_from, date_to, limit -> item dengan spend terbesar. Contoh: "10 item kemasan dengan spend terbesar tahun ini?"
6. alerts: kind (price|vendor|all), date_from, date_to? -> daftar alert. Contoh: "Ada alert harga apa saja minggu ini?"
7. kpi_summary: date_from, date_to -> total spend, jumlah PO, vendor aktif, on-time vs periode sebelumnya.
8. none: pertanyaan di luar data SpendView (mis. harga emas, cuaca, saham, kurs) atau tidak bisa dijawab dengan tool di atas.

Aturan: tanggal format YYYY-MM-DD. Gunakan nama kategori/komoditas dari daftar di atas. Isi "reason" dengan alasan singkat.
{fb}
Pertanyaan user: {question}"#,
        today = ctx.reference,
        week = iso_week_label(ctx.reference),
        ws = ws,
        ms = ms,
        year = ctx.reference.format("%Y"),
        cats = ctx.categories.join(", "),
        coms = ctx.commodities.join(", "),
        fb = fb,
        question = question.trim(),
    )
}

pub fn table_text(r: &ToolResult) -> String {
    let mut s = format!("{}\nKolom: {}\n", r.title, r.columns.join(" | "));
    for row in r.rows.iter().take(60) {
        let cells: Vec<String> = row.iter().map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Null => "-".into(),
            other => other.to_string(),
        }).collect();
        s.push_str(&cells.join(" | "));
        s.push('\n');
    }
    if let Some(n) = &r.note {
        s.push_str(&format!("Catatan: {n}\n"));
    }
    s
}

pub fn summary_prompt(question: &str, r: &ToolResult, feedback: Option<&str>) -> String {
    let fb = feedback
        .map(|f| format!("\nRINGKASAN SEBELUMNYA DITOLAK karena memuat angka yang tidak ada di tabel: {f}. Gunakan hanya angka dari tabel.\n"))
        .unwrap_or_default();
    format!(
        "Kamu analis procurement. Jawab pertanyaan user dalam Bahasa Indonesia, 2-4 kalimat, berdasarkan TABEL HASIL di bawah.\n\
ATURAN KERAS: setiap angka yang kamu tulis HARUS ada di tabel atau catatan (boleh dibulatkan, mis. Rp 1.234.567.890 -> Rp 1,23 miliar). \
Jangan menghitung angka baru (selisih, total, rata-rata) yang tidak ada di tabel. Jangan menambah informasi dari luar tabel.\n{fb}\n\
Pertanyaan: {question}\n\nTABEL HASIL (tool {tool}):\n{table}",
        question = question.trim(),
        tool = r.tool,
        table = table_text(r),
    )
}

pub fn explain_prompt(category: &str, facts: &ToolResult, feedback: Option<&str>) -> String {
    let fb = feedback
        .map(|f| format!("\nNARASI SEBELUMNYA DITOLAK karena memuat angka yang tidak ada di tabel: {f}.\n"))
        .unwrap_or_default();
    format!(
        "Kamu analis procurement. Tulis narasi singkat (3-4 kalimat, Bahasa Indonesia) yang menjelaskan KENAPA spend kategori {category} \
naik/turun dibanding periode sebelumnya, berdasarkan dekomposisi harga vs volume berikut. Sebutkan efek terbesar dan item penyumbang utama.\n\
ATURAN KERAS: semua angka harus berasal dari tabel (boleh dibulatkan, mis. Rp 1,2 miliar). Jangan menghitung angka baru.\n{fb}\n{table}",
        table = table_text(facts),
    )
}

// ---------------------------------------------------------------- validasi

#[derive(Debug, Clone)]
pub enum Validated {
    SpendBy { group: GroupBy, from: NaiveDate, to: NaiveDate, scope: Scope, cat_name: Option<String>, vendor_name: Option<String>, limit: i64 },
    CommodityPrice { id: i64, name: String, unit: String, weeks: i64 },
    VendorRanking { metric: String, desc: bool, limit: i64, from: NaiveDate, to: NaiveDate, scope: Scope, scope_label: Option<String> },
    PriceChange { item: Option<i64>, commodity: Option<i64>, name: String, unit: String, a: (NaiveDate, NaiveDate, String), b: (NaiveDate, NaiveDate, String) },
    TopItems { from: NaiveDate, to: NaiveDate, cat: Option<(i64, String)>, limit: i64 },
    Alerts { kind: Option<String>, from: NaiveDate, to: NaiveDate },
    Kpi { from: NaiveDate, to: NaiveDate },
    None,
}

fn date_or(s: &Option<String>, def: NaiveDate, name: &str) -> Result<NaiveDate, String> {
    match s.as_deref().map(str::trim) {
        None | Some("") => Ok(def),
        Some(v) => parse_date(v).ok_or_else(|| format!("{name} '{v}' harus berformat YYYY-MM-DD")),
    }
}

fn range(p: &ToolParams, ctx: &ToolContext) -> Result<(NaiveDate, NaiveDate), String> {
    let from = date_or(&p.date_from, ctx.default_from, "date_from")?;
    let to = date_or(&p.date_to, ctx.default_to, "date_to")?.min(ctx.reference.max(from));
    if from > to {
        return Err("date_from harus sebelum date_to".into());
    }
    Ok((from, to))
}

fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

pub async fn validate(db: &SqlitePool, ctx: &ToolContext, c: &ToolChoice) -> AppResult<Result<Validated, String>> {
    let p = &c.params;
    let limit = |def: i64| p.limit.unwrap_or(def).clamp(1, 50);
    let v = match c.tool.as_str() {
        "none" => Ok(Validated::None),
        "spend_by" => {
            let group = match non_empty(&p.group_by) {
                Some("category") => Ok(GroupBy::Category),
                Some("vendor") => Ok(GroupBy::Vendor),
                Some("item") => Ok(GroupBy::Item),
                Some("month") => Ok(GroupBy::Month),
                other => Err(format!("group_by '{}' tidak valid (category|vendor|item|month)", other.unwrap_or("null"))),
            };
            match (group, range(p, ctx)) {
                (Ok(group), Ok((from, to))) => {
                    let mut scope = Scope::default();
                    let mut cat_name = None;
                    let mut vendor_name = None;
                    let mut err = None;
                    if let Some(cn) = non_empty(&p.category) {
                        match queries::find_category(db, cn).await? {
                            Some((id, n)) => {
                                scope.category = Some(id);
                                cat_name = Some(n);
                            }
                            None => err = Some(format!("kategori '{cn}' tidak ada")),
                        }
                    }
                    if let Some(vn) = non_empty(&p.vendor) {
                        match queries::find_vendor(db, vn).await? {
                            Some((id, n)) => {
                                scope.vendor = Some(id);
                                vendor_name = Some(n);
                            }
                            None => err = Some(format!("vendor '{vn}' tidak ada")),
                        }
                    }
                    match err {
                        Some(e) => Err(e),
                        None => Ok(Validated::SpendBy { group, from, to, scope, cat_name, vendor_name, limit: limit(10) }),
                    }
                }
                (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        "commodity_price" => match non_empty(&p.commodity) {
            None => Err("commodity wajib diisi".into()),
            Some(cn) => match queries::find_commodity(db, cn).await? {
                None => Err(format!("komoditas '{cn}' tidak ada. Pilihan: {}", ctx.commodities.join(", "))),
                Some((id, name, unit)) => Ok(Validated::CommodityPrice { id, name, unit, weeks: p.weeks.unwrap_or(8).clamp(1, 52) }),
            },
        },
        "vendor_ranking" => {
            let metric = non_empty(&p.metric).unwrap_or("score").to_string();
            if !["score", "on_time", "late", "quality", "price"].contains(&metric.as_str()) {
                Err(format!("metric '{metric}' tidak valid"))
            } else {
                match range(p, ctx) {
                    Err(e) => Err(e),
                    Ok((from, to)) => {
                        let mut scope = Scope::default();
                        let mut label = None;
                        let mut err = None;
                        if let Some(cn) = non_empty(&p.commodity) {
                            match queries::find_commodity(db, cn).await? {
                                Some((id, n, _)) => {
                                    scope.commodity = Some(id);
                                    label = Some(format!("komoditas {n}"));
                                }
                                None => err = Some(format!("komoditas '{cn}' tidak ada")),
                            }
                        }
                        if let Some(cn) = non_empty(&p.category) {
                            match queries::find_category(db, cn).await? {
                                Some((id, n)) => {
                                    scope.category = Some(id);
                                    label = Some(format!("kategori {n}"));
                                }
                                None => err = Some(format!("kategori '{cn}' tidak ada")),
                            }
                        }
                        let default_desc = metric != "price";
                        let desc = match non_empty(&p.order) {
                            Some("asc") => false,
                            Some("desc") => true,
                            _ => default_desc,
                        };
                        match err {
                            Some(e) => Err(e),
                            None => Ok(Validated::VendorRanking { metric, desc, limit: limit(5), from, to, scope, scope_label: label }),
                        }
                    }
                }
            }
        }
        "price_change" => {
            let pa = non_empty(&p.period_a).and_then(|s| parse_period(s).map(|r| (r.0, r.1, s.to_string())));
            let pb = non_empty(&p.period_b).and_then(|s| parse_period(s).map(|r| (r.0, r.1, s.to_string())));
            match (pa, pb) {
                (Some(a), Some(b)) => {
                    let b = (b.0, b.1.min(ctx.reference), b.2);
                    if let Some(cn) = non_empty(&p.commodity) {
                        match queries::find_commodity(db, cn).await? {
                            Some((id, name, unit)) => Ok(Validated::PriceChange { item: None, commodity: Some(id), name, unit, a, b }),
                            None => Err(format!("komoditas '{cn}' tidak ada")),
                        }
                    } else if let Some(iname) = non_empty(&p.item) {
                        match queries::find_item(db, iname).await? {
                            Some((id, name, unit)) => Ok(Validated::PriceChange { item: Some(id), commodity: None, name, unit, a, b }),
                            None => match queries::find_commodity(db, iname).await? {
                                Some((id, name, unit)) => Ok(Validated::PriceChange { item: None, commodity: Some(id), name, unit, a, b }),
                                None => Err(format!("item '{iname}' tidak ada")),
                            },
                        }
                    } else {
                        Err("isi item atau commodity".into())
                    }
                }
                _ => Err("period_a dan period_b wajib, format YYYY-MM atau YYYY-Www".into()),
            }
        }
        "top_items" => match range(p, ctx) {
            Err(e) => Err(e),
            Ok((from, to)) => match non_empty(&p.category) {
                None => Ok(Validated::TopItems { from, to, cat: None, limit: limit(10) }),
                Some(cn) => match queries::find_category(db, cn).await? {
                    Some(c) => Ok(Validated::TopItems { from, to, cat: Some(c), limit: limit(10) }),
                    None => Err(format!("kategori '{cn}' tidak ada. Pilihan: {}", ctx.categories.join(", "))),
                },
            },
        },
        "alerts" => {
            let kind = match non_empty(&p.kind) {
                None | Some("all") => Ok(None),
                Some("price") => Ok(Some("price".to_string())),
                Some("vendor") => Ok(Some("vendor".to_string())),
                Some(k) => Err(format!("kind '{k}' tidak valid")),
            };
            let from = date_or(&p.date_from, week_start(ctx.reference), "date_from");
            let to = date_or(&p.date_to, ctx.reference, "date_to");
            match (kind, from, to) {
                (Ok(kind), Ok(from), Ok(to)) => Ok(Validated::Alerts { kind, from, to }),
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
            }
        }
        "kpi_summary" => range(p, ctx).map(|(from, to)| Validated::Kpi { from, to }),
        other => Err(format!("tool '{other}' tidak dikenal")),
    };
    Ok(v)
}

// ---------------------------------------------------------------- eksekusi

fn r0(x: f64) -> Value {
    json!(x.round())
}
fn r1(x: f64) -> Value {
    json!((x * 10.0).round() / 10.0)
}
fn o1(x: Option<f64>) -> Value {
    x.map(r1).unwrap_or(Value::Null)
}
fn group_label(g: GroupBy) -> &'static str {
    match g {
        GroupBy::Category => "Kategori",
        GroupBy::Vendor => "Vendor",
        GroupBy::Item => "Item",
        GroupBy::Month => "Bulan",
    }
}

pub async fn execute(db: &SqlitePool, ctx: &ToolContext, v: &Validated, tool: &str) -> AppResult<ToolResult> {
    let mut r = ToolResult { tool: tool.to_string(), params: Value::Null, title: String::new(), columns: vec![], rows: vec![], note: None };
    match v {
        Validated::None => {
            r.title = "Di luar cakupan data SpendView".into();
        }
        Validated::SpendBy { group, from, to, scope, cat_name, vendor_name, limit } => {
            r.params = json!({ "group_by": group, "date_from": from, "date_to": to, "category": cat_name, "vendor": vendor_name, "limit": limit });
            let rows = queries::spend_by(db, *group, *from, *to, *scope, Some(if *group == GroupBy::Month { 60 } else { *limit })).await?;
            let total = queries::kpis(db, *from, *to, *scope).await?.total_spend;
            r.title = format!("Spend per {} ({} s/d {})", group_label(*group).to_lowercase(), from, to);
            if *group == GroupBy::Month {
                r.columns = vec!["Bulan".into(), "Spend (Rp)".into(), "Jumlah PO".into()];
                r.rows = rows.iter().map(|x| vec![json!(x.name), r0(x.spend), json!(x.po_count)]).collect();
            } else {
                r.columns = vec!["Peringkat".into(), group_label(*group).into(), "Spend (Rp)".into(), "Jumlah PO".into(), "Porsi (%)".into()];
                r.rows = rows.iter().enumerate().map(|(i, x)| vec![json!(i + 1), json!(x.name), r0(x.spend), json!(x.po_count), r1(x.share_pct)]).collect();
            }
            r.note = Some(format!("Total spend periode ini: {}", fmt_rp(total)));
        }
        Validated::CommodityPrice { id, name, unit, weeks } => {
            r.params = json!({ "commodity": name, "weeks": weeks });
            let d = queries::commodity_detail(db, *id, ctx.reference, *weeks, ctx.anomaly).await?;
            r.title = format!("Harga rata-rata tertimbang {} per minggu ({} minggu terakhir)", name, weeks);
            r.columns = vec!["Minggu".into(), "Mulai".into(), format!("Harga (Rp/{unit})"), "Perubahan vs minggu lalu (%)".into(), format!("Qty ({unit})"), "Alert".into()];
            r.rows = d
                .weeks
                .iter()
                .map(|w| vec![json!(w.week), json!(w.week_start), w.price.map(r0).unwrap_or(Value::Null), o1(w.change_pct), r0(w.qty), json!(if w.alert { "ya" } else { "-" })])
                .collect();
            let prices: Vec<(String, f64)> = d.weeks.iter().filter_map(|w| w.price.map(|p| (w.week.clone(), p))).collect();
            if let (Some(first), Some(last)) = (prices.first(), prices.last()) {
                if prices.len() > 1 && first.1 > 0.0 {
                    let ch = (last.1 / first.1 - 1.0) * 100.0;
                    r.note = Some(format!(
                        "Perubahan {} -> {}: {} ({} -> {} per {unit}); arah: {}",
                        first.0,
                        last.0,
                        fmt_pct(ch),
                        fmt_rp(first.1),
                        fmt_rp(last.1),
                        if ch > 0.5 { "naik" } else if ch < -0.5 { "turun" } else { "stabil" }
                    ));
                }
            }
        }
        Validated::VendorRanking { metric, desc, limit, from, to, scope, scope_label } => {
            r.params = json!({ "metric": metric, "order": if *desc { "desc" } else { "asc" }, "limit": limit, "date_from": from, "date_to": to, "scope": scope_label });
            let mut rows = queries::vendor_scorecard(db, *from, *to, *scope, ctx.weights).await?;
            let key = |x: &queries::VendorScoreRow| -> f64 {
                match metric.as_str() {
                    "on_time" => x.on_time_pct.unwrap_or(-1.0),
                    "late" => x.late as f64,
                    "quality" => x.quality_score.unwrap_or(-1.0),
                    "price" => x.price_diff_pct.unwrap_or(0.0),
                    _ => x.total.unwrap_or(-1.0),
                }
            };
            rows.sort_by(|a, b| if *desc { key(b).total_cmp(&key(a)) } else { key(a).total_cmp(&key(b)) });
            rows.truncate(*limit as usize);
            r.title = format!(
                "Peringkat vendor berdasarkan {metric}{} ({} s/d {})",
                scope_label.as_ref().map(|s| format!(" untuk {s}")).unwrap_or_default(),
                from,
                to
            );
            r.columns = vec![
                "Peringkat".into(), "Vendor".into(), "Skor".into(), "Label".into(), "Tepat waktu (%)".into(), "PO telat".into(),
                "PO terkirim".into(), "Skor kualitas".into(), "Selisih harga vs pasar (%)".into(),
            ];
            r.rows = rows
                .iter()
                .enumerate()
                .map(|(i, x)| vec![json!(i + 1), json!(x.vendor), o1(x.total), json!(x.label), o1(x.on_time_pct), json!(x.late), json!(x.delivered), o1(x.quality_score), o1(x.price_diff_pct)])
                .collect();
        }
        Validated::PriceChange { item, commodity, name, unit, a, b } => {
            r.params = json!({ "name": name, "period_a": a.2, "period_b": b.2 });
            let pc = queries::price_change(db, *item, *commodity, name, unit, a.clone(), b.clone()).await?;
            r.title = format!("Perubahan harga {} ({} vs {})", name, a.2, b.2);
            r.columns = vec![
                "Nama".into(), "Periode A".into(), format!("Harga A (Rp/{unit})"), "Periode B".into(), format!("Harga B (Rp/{unit})"),
                "Perubahan (%)".into(), format!("Qty A ({unit})"), format!("Qty B ({unit})"),
            ];
            if pc.price_a.is_some() || pc.price_b.is_some() {
                r.rows = vec![vec![
                    json!(name), json!(format!("{} ({} s/d {})", a.2, a.0, a.1)), pc.price_a.map(r0).unwrap_or(Value::Null),
                    json!(format!("{} ({} s/d {})", b.2, b.0, b.1)), pc.price_b.map(r0).unwrap_or(Value::Null), o1(pc.change_pct), r0(pc.qty_a), r0(pc.qty_b),
                ]];
            }
        }
        Validated::TopItems { from, to, cat, limit } => {
            r.params = json!({ "category": cat.as_ref().map(|c| c.1.clone()), "date_from": from, "date_to": to, "limit": limit });
            let scope = Scope { category: cat.as_ref().map(|c| c.0), ..Default::default() };
            let rows = queries::top_items(db, *from, *to, scope, *limit).await?;
            r.title = format!("Top {} item{} berdasarkan spend ({} s/d {})", limit, cat.as_ref().map(|c| format!(" {}", c.1)).unwrap_or_default(), from, to);
            r.columns = vec!["Peringkat".into(), "Item".into(), "Spend (Rp)".into(), "Qty".into(), "Satuan".into(), "Harga rata-rata (Rp)".into(), "Jumlah PO".into()];
            r.rows = rows
                .iter()
                .enumerate()
                .map(|(i, x)| vec![json!(i + 1), json!(x.item), r0(x.spend), r0(x.base_qty), json!(x.base_unit), r0(x.avg_price), json!(x.po_count)])
                .collect();
        }
        Validated::Alerts { kind, from, to } => {
            r.params = json!({ "kind": kind.clone().unwrap_or_else(|| "all".into()), "date_from": from, "date_to": to });
            let rows = queries::alerts(db, kind.as_deref(), Some(*from), Some(*to), 50).await?;
            r.title = format!("Alert {} periode {} s/d {}", kind.as_deref().map(|k| if k == "price" { "harga" } else { "vendor" }).unwrap_or("semua"), from, to);
            r.columns = vec!["Minggu/Periode".into(), "Jenis".into(), "Komoditas/Vendor".into(), "Pesan".into(), "Nilai (%)".into()];
            r.rows = rows
                .iter()
                .map(|a| vec![json!(a.week), json!(if a.kind == "price" { "harga" } else { "vendor" }), json!(a.ref_name), json!(a.message), r1(a.value)])
                .collect();
        }
        Validated::Kpi { from, to } => {
            r.params = json!({ "date_from": from, "date_to": to });
            let (pf, pt) = previous_period(*from, *to);
            let cur = queries::kpis(db, *from, *to, Scope::default()).await?;
            let prev = queries::kpis(db, pf, pt, Scope::default()).await?;
            let ch = |a: f64, b: f64| if b > 0.0 { r1((a / b - 1.0) * 100.0) } else { Value::Null };
            r.title = format!("Ringkasan KPI {} s/d {} vs {} s/d {}", from, to, pf, pt);
            r.columns = vec!["Metrik".into(), "Periode ini".into(), "Periode sebelumnya".into(), "Perubahan (%)".into()];
            r.rows = vec![
                vec![json!("Total spend (Rp)"), r0(cur.total_spend), r0(prev.total_spend), ch(cur.total_spend, prev.total_spend)],
                vec![json!("Jumlah PO"), json!(cur.po_count), json!(prev.po_count), ch(cur.po_count as f64, prev.po_count as f64)],
                vec![json!("Vendor aktif"), json!(cur.active_vendors), json!(prev.active_vendors), ch(cur.active_vendors as f64, prev.active_vendors as f64)],
                vec![json!("On-time delivery (%)"), o1(cur.on_time_pct), o1(prev.on_time_pct), Value::Null],
            ];
        }
    }
    Ok(r)
}

// ---------------------------------------------------------------- ringkasan cadangan

fn num(v: &Value) -> Option<f64> {
    v.as_f64()
}
fn s(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())
}

/// Ringkasan deterministik dari tabel (dipakai mock provider dan sebagai cadangan kalau
/// ringkasan AI memuat angka yang tidak bisa diverifikasi).
pub fn template_summary(r: &ToolResult) -> String {
    if r.tool == "none" {
        return "Maaf, data untuk pertanyaan itu tidak tersedia di SpendView. SpendView hanya berisi data PO, harga komoditas internal, penilaian vendor, dan alert.".into();
    }
    if r.rows.is_empty() {
        return format!("Tidak ada data untuk {}.", r.title.to_lowercase());
    }
    let first = &r.rows[0];
    match r.tool.as_str() {
        "spend_by" if r.columns.first().map(|c| c == "Bulan").unwrap_or(false) => {
            let max = r.rows.iter().max_by(|a, b| num(&a[1]).unwrap_or(0.0).total_cmp(&num(&b[1]).unwrap_or(0.0)));
            match max {
                Some(m) => format!("{}. Bulan dengan spend tertinggi adalah {} sebesar {}.", r.title, s(&m[0]), fmt_rp(num(&m[1]).unwrap_or(0.0))),
                None => r.title.clone(),
            }
        }
        "spend_by" => {
            let mut t = format!(
                "{} dengan spend terbesar pada {} adalah {} sebesar {} ({} dari total, {} PO).",
                r.columns[1],
                r.title.split('(').nth(1).map(|x| x.trim_end_matches(')')).unwrap_or(""),
                s(&first[1]),
                fmt_rp(num(&first[2]).unwrap_or(0.0)),
                fmt_pct(num(&first[4]).unwrap_or(0.0)),
                s(&first[3])
            );
            if let Some(second) = r.rows.get(1) {
                t.push_str(&format!(" Di posisi kedua {} dengan {}.", s(&second[1]), fmt_rp(num(&second[2]).unwrap_or(0.0))));
            }
            if let Some(n) = &r.note {
                t.push_str(&format!(" {n}."));
            }
            t
        }
        "commodity_price" => {
            let mut t = format!("{}.", r.title);
            if let Some(n) = &r.note {
                t.push_str(&format!(" {}.", n));
            }
            if let Some(last) = r.rows.iter().rev().find(|row| !row[2].is_null()) {
                t.push_str(&format!(" Minggu {} harga rata-rata {}", s(&last[0]), fmt_rp(num(&last[2]).unwrap_or(0.0))));
                if let Some(c) = num(&last[3]) {
                    t.push_str(&format!(" ({} vs minggu lalu)", fmt_pct(c)));
                }
                t.push('.');
            }
            let alerts: Vec<String> = r.rows.iter().filter(|row| row[5] == json!("ya")).map(|row| s(&row[0])).collect();
            if !alerts.is_empty() {
                t.push_str(&format!(" Alert harga muncul di minggu: {}.", alerts.join(", ")));
            }
            t
        }
        "vendor_ranking" => {
            let mut t = format!("{}: posisi teratas {} (skor {}, label {}, tepat waktu {}, {} PO telat dari {} PO terkirim).",
                r.title,
                s(&first[1]),
                num(&first[2]).map(|x| format!("{x:.1}").replace('.', ",")).unwrap_or("-".into()),
                s(&first[3]),
                num(&first[4]).map(fmt_pct).unwrap_or("-".into()),
                s(&first[5]),
                s(&first[6]));
            if let Some(second) = r.rows.get(1) {
                t.push_str(&format!(" Berikutnya {} ({} PO telat, tepat waktu {}).", s(&second[1]), s(&second[5]), num(&second[4]).map(fmt_pct).unwrap_or("-".into())));
            }
            t
        }
        "price_change" => match (num(&first[2]), num(&first[4]), num(&first[5])) {
            (Some(a), Some(b), Some(c)) => format!(
                "Harga rata-rata tertimbang {} {} {} dari {} ({}) menjadi {} ({}).",
                s(&first[0]),
                if c >= 0.0 { "naik" } else { "turun" },
                fmt_pct(c.abs()),
                fmt_rp(a),
                s(&first[1]),
                fmt_rp(b),
                s(&first[3])
            ),
            _ => format!("Data harga {} tidak lengkap untuk kedua periode.", s(&first[0])),
        },
        "top_items" => {
            let names: Vec<String> = r.rows.iter().take(3).map(|row| format!("{} ({})", s(&row[1]), fmt_rp(num(&row[2]).unwrap_or(0.0)))).collect();
            format!("{}. Tiga teratas: {}.", r.title, names.join(", "))
        }
        "alerts" => {
            let items: Vec<String> = r.rows.iter().take(5).map(|row| s(&row[3])).collect();
            format!("Ada {} alert pada {}: {}", r.rows.len(), r.title.to_lowercase().replace("alert ", ""), items.join(" | "))
        }
        "kpi_summary" => format!(
            "{}: total spend {} (periode sebelumnya {}), {} PO, {} vendor aktif, on-time delivery {}.",
            r.title,
            fmt_rp(num(&r.rows[0][1]).unwrap_or(0.0)),
            fmt_rp(num(&r.rows[0][2]).unwrap_or(0.0)),
            s(&r.rows[1][1]),
            s(&r.rows[2][1]),
            num(&r.rows[3][1]).map(fmt_pct).unwrap_or("-".into())
        ),
        _ => r.title.clone(),
    }
}

pub const EXAMPLE_QUESTIONS: [&str; 7] = [
    "Kategori apa yang paling besar kuartal ini?",
    "Harga gula 8 minggu terakhir gimana?",
    "Vendor gula mana yang paling sering telat bulan ini?",
    "Tepung naik berapa persen dibanding bulan lalu?",
    "10 item kemasan dengan spend terbesar tahun ini?",
    "Ada alert harga apa saja minggu ini?",
    "Harga emas hari ini berapa?",
];
