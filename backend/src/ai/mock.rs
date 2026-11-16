//! Provider AI tiruan (AI_PROVIDER=mock). Tidak memanggil apa pun: memilih tool dengan
//! aturan kata kunci sederhana dan membuat ringkasan dari template. Dipakai untuk test dan
//! kalau Claude/Codex CLI tidak tersedia, dengan struktur output yang sama persis.

use super::tools::{template_summary, ToolChoice, ToolParams, ToolResult};
use super::{AiOutput, AiProvider, AiPurpose, AiRequest, BoxFut};
use crate::aggregates::{fmt_pct, fmt_rp};
use crate::analytics::period::{add_months, iso_week_label, month_end, month_start, week_start};
use chrono::{Datelike, Duration, NaiveDate};
use serde_json::{json, Value};

pub struct MockAi;

impl AiProvider for MockAi {
    fn name(&self) -> &'static str {
        "mock"
    }
    fn complete<'a>(&'a self, req: &'a AiRequest) -> BoxFut<'a> {
        Box::pin(async move { respond(req) })
    }
}

fn respond(req: &AiRequest) -> AiOutput {
    let v = match &req.purpose {
        AiPurpose::SelectTool { question, reference_date } => {
            serde_json::to_value(route(question, *reference_date)).unwrap_or(Value::Null)
        }
        AiPurpose::Summarize { result } => json!({ "summary": template_summary(result) }),
        AiPurpose::Explain { facts } => json!({ "narrative": explain_template(facts) }),
    };
    Ok((v.to_string(), v))
}

fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|w| q.contains(w))
}

fn first_number(q: &str) -> Option<i64> {
    q.split(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty()).and_then(|s| s.parse().ok())
}

const COMMODITY_WORDS: [(&str, &str); 8] = [
    ("gula", "Gula Pasir"), ("tepung", "Tepung Terigu"), ("minyak", "Minyak Nabati"), ("mentega", "Mentega"),
    ("susu", "Susu Bubuk"), ("cokelat", "Cokelat Bubuk"), ("keju", "Keju"), ("telur", "Telur Ayam"),
];
const CATEGORY_WORDS: [(&str, &str); 6] = [
    ("bahan baku", "Bahan Baku"), ("kemasan", "Kemasan"), ("sparepart", "Sparepart"), ("jasa", "Jasa"), ("logistik", "Logistik"),
    ("pendukung", "Bahan Pendukung"),
];

/// Rentang tanggal dari frasa waktu relatif terhadap tanggal acuan.
fn time_range(q: &str, today: NaiveDate) -> (Option<NaiveDate>, Option<NaiveDate>) {
    if q.contains("minggu ini") {
        (Some(week_start(today)), Some(today))
    } else if q.contains("minggu lalu") {
        let ws = week_start(today) - Duration::days(7);
        (Some(ws), Some(ws + Duration::days(6)))
    } else if q.contains("bulan lalu") {
        let s = add_months(month_start(today), -1);
        (Some(s), Some(month_end(s)))
    } else if q.contains("bulan ini") {
        (Some(month_start(today)), Some(today))
    } else if q.contains("kuartal ini") {
        let qm = (today.month0() / 3) * 3 + 1;
        (NaiveDate::from_ymd_opt(today.year(), qm, 1), Some(today))
    } else if q.contains("tahun ini") {
        (NaiveDate::from_ymd_opt(today.year(), 1, 1), Some(today))
    } else if q.contains("bulan terakhir") {
        let n = first_number(q).unwrap_or(3) as i32;
        (Some(add_months(month_start(today), -(n - 1))), Some(today))
    } else {
        (None, None)
    }
}

pub fn route(question: &str, today: NaiveDate) -> ToolChoice {
    let q = question.to_lowercase();
    let commodity = COMMODITY_WORDS.iter().find(|(w, _)| q.contains(w)).map(|(_, n)| n.to_string());
    let category = CATEGORY_WORDS.iter().find(|(w, _)| q.contains(w)).map(|(_, n)| n.to_string());
    let (from, to) = time_range(&q, today);
    let mut p = ToolParams { date_from: from.map(|d| d.to_string()), date_to: to.map(|d| d.to_string()), ..Default::default() };
    let choose = |tool: &str, params: ToolParams, reason: &str| ToolChoice { tool: tool.into(), params, reason: reason.into() };

    let outside = ["emas", "saham", "cuaca", "kurs", "dolar", "dollar", "bitcoin", "crypto", "presiden", "bola"];
    let words: Vec<&str> = q.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    if outside.iter().any(|o| words.contains(o)) {
        return choose("none", ToolParams::default(), "pertanyaan di luar data SpendView");
    }
    if q.contains("alert") || q.contains("peringatan") {
        p.kind = Some(if q.contains("vendor") { "vendor" } else if q.contains("harga") { "price" } else { "all" }.into());
        if p.date_from.is_none() {
            p.date_from = Some(week_start(today).to_string());
        }
        return choose("alerts", p, "pertanyaan tentang alert");
    }
    if q.contains("vendor") && has(&q, &["telat", "terlambat", "tepat waktu", "on-time", "on time", "terbaik", "terburuk", "skor", "bermasalah", "kualitas", "reject", "termurah", "termahal"]) {
        let (metric, order) = if has(&q, &["telat", "terlambat"]) {
            ("late", "desc")
        } else if has(&q, &["tepat waktu", "on-time", "on time"]) {
            ("on_time", "desc")
        } else if has(&q, &["kualitas", "reject"]) {
            ("quality", if q.contains("buruk") || q.contains("reject") { "asc" } else { "desc" })
        } else if has(&q, &["termurah", "termahal"]) {
            ("price", if q.contains("termahal") { "desc" } else { "asc" })
        } else if has(&q, &["terburuk", "bermasalah"]) {
            ("score", "asc")
        } else {
            ("score", "desc")
        };
        p.metric = Some(metric.into());
        p.order = Some(order.into());
        p.limit = Some(first_number(&q).unwrap_or(5));
        p.commodity = commodity;
        p.category = category;
        return choose("vendor_ranking", p, "peringkat vendor");
    }
    if has(&q, &["naik berapa", "turun berapa", "berapa persen", "dibanding", "dibandingkan", " vs "]) && (commodity.is_some() || q.contains("harga")) {
        let this_m = month_start(today);
        let (a, b) = if q.contains("minggu lalu") {
            (iso_week_label(today - Duration::days(7)), iso_week_label(today))
        } else {
            (add_months(this_m, -1).format("%Y-%m").to_string(), this_m.format("%Y-%m").to_string())
        };
        return choose(
            "price_change",
            ToolParams { commodity, period_a: Some(a), period_b: Some(b), ..Default::default() },
            "perubahan harga antar periode",
        );
    }
    if q.contains("harga") && commodity.is_some() {
        let weeks = if q.contains("minggu") { first_number(&q).unwrap_or(8) } else if q.contains("bulan") { first_number(&q).unwrap_or(3) * 4 } else { 8 };
        return choose("commodity_price", ToolParams { commodity, weeks: Some(weeks), ..Default::default() }, "tren harga komoditas");
    }
    if q.contains("item") || q.contains("barang") {
        p.category = category;
        p.limit = Some(first_number(&q).unwrap_or(10));
        return choose("top_items", p, "item dengan spend terbesar");
    }
    if q.contains("kategori") {
        p.group_by = Some("category".into());
        return choose("spend_by", p, "spend per kategori");
    }
    if q.contains("vendor") && has(&q, &["spend", "belanja", "pengeluaran", "terbesar", "paling besar"]) {
        p.group_by = Some("vendor".into());
        p.category = category;
        return choose("spend_by", p, "spend per vendor");
    }
    if has(&q, &["per bulan", "bulanan", "tren spend"]) {
        p.group_by = Some("month".into());
        return choose("spend_by", p, "tren spend bulanan");
    }
    if has(&q, &["total spend", "kpi", "ringkasan", "berapa spend", "total belanja", "jumlah po"]) {
        return choose("kpi_summary", p, "ringkasan KPI");
    }
    if let Some(c) = category {
        p.group_by = Some("item".into());
        p.category = Some(c);
        return choose("spend_by", p, "spend item dalam kategori");
    }
    choose("none", ToolParams::default(), "tidak ada tool yang cocok")
}

/// Narasi cadangan untuk "Jelaskan dengan AI" dari tabel dekomposisi.
pub fn explain_template(facts: &ToolResult) -> String {
    let get = |label: &str| -> Option<f64> {
        facts.rows.iter().find(|r| r.first().and_then(Value::as_str) == Some(label)).and_then(|r| r.get(1)).and_then(Value::as_f64)
    };
    let (Some(s0), Some(s1), Some(total), Some(vol), Some(price)) =
        (get("Spend periode sebelumnya"), get("Spend periode ini"), get("Total perubahan"), get("Efek volume"), get("Efek harga"))
    else {
        return "Data dekomposisi tidak lengkap.".into();
    };
    let dir = if total >= 0.0 { "naik" } else { "turun" };
    let main = if vol.abs() >= price.abs() { "perubahan volume pembelian" } else { "perubahan harga" };
    let mut t = format!(
        "Spend kategori ini {dir} dari {} menjadi {} (selisih {}). Penyebab utamanya adalah {main}: efek volume {} dan efek harga {}.",
        fmt_rp(s0),
        fmt_rp(s1),
        fmt_rp(total),
        fmt_rp(vol),
        fmt_rp(price)
    );
    let items: Vec<String> = facts
        .rows
        .iter()
        .filter(|r| r.first().and_then(Value::as_str).map(|s| s.starts_with("Item: ")).unwrap_or(false))
        .take(2)
        .filter_map(|r| {
            let name = r.first()?.as_str()?.trim_start_matches("Item: ").to_string();
            let ch = r.get(1)?.as_f64()?;
            let pp = r.get(4).and_then(Value::as_f64);
            Some(match pp {
                Some(p) => format!("{name} ({}, harga {})", fmt_rp(ch), fmt_pct(p)),
                None => format!("{name} ({})", fmt_rp(ch)),
            })
        })
        .collect();
    if !items.is_empty() {
        t.push_str(&format!(" Item penyumbang perubahan terbesar: {}.", items.join(" dan ")));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 15).expect("valid")
    }

    #[test]
    fn routing_contoh_pertanyaan() {
        let c = route("Kategori apa yang paling besar kuartal ini?", today());
        assert_eq!(c.tool, "spend_by");
        assert_eq!(c.params.group_by.as_deref(), Some("category"));
        assert_eq!(c.params.date_from.as_deref(), Some("2026-10-01"));

        let c = route("Harga gula 8 minggu terakhir gimana?", today());
        assert_eq!((c.tool.as_str(), c.params.weeks), ("commodity_price", Some(8)));

        let c = route("Vendor gula mana yang paling sering telat bulan ini?", today());
        assert_eq!(c.tool, "vendor_ranking");
        assert_eq!(c.params.metric.as_deref(), Some("late"));
        assert_eq!(c.params.commodity.as_deref(), Some("Gula Pasir"));

        let c = route("Tepung naik berapa persen dibanding bulan lalu?", today());
        assert_eq!(c.tool, "price_change");
        assert_eq!(c.params.period_a.as_deref(), Some("2026-10"));
        assert_eq!(c.params.period_b.as_deref(), Some("2026-11"));

        let c = route("10 item kemasan dengan spend terbesar tahun ini?", today());
        assert_eq!((c.tool.as_str(), c.params.limit), ("top_items", Some(10)));
        assert_eq!(c.params.category.as_deref(), Some("Kemasan"));
        assert_eq!(c.params.date_from.as_deref(), Some("2026-01-01"));

        let c = route("Ada alert harga apa saja minggu ini?", today());
        assert_eq!((c.tool.as_str(), c.params.kind.as_deref()), ("alerts", Some("price")));
        assert_eq!(c.params.date_from.as_deref(), Some("2026-11-09"));

        assert_eq!(route("harga emas hari ini?", today()).tool, "none");
    }
}
