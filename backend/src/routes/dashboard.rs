//! Endpoint dashboard: meta/filter, overview, spend, komoditas, vendor, alert.

use crate::ai::numcheck::{allowed_numbers, unverified_numbers};
use crate::ai::tools::{explain_prompt, explain_schema, ExplainOut, ToolResult};
use crate::ai::{AiPurpose, AiRequest};
use crate::analytics::period::{add_months, month_end, month_start, parse_date};
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::filters::{default_range, reference_date, resolve, Filter, FilterQuery};
use crate::queries::{self, Scope};
use crate::settings::Settings;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

fn scope(f: &Filter) -> Scope {
    Scope { category: f.category, vendor: f.vendor, commodity: None }
}

pub async fn meta(State(st): State<AppState>, _u: CurrentUser) -> AppResult<Json<Value>> {
    let reference = reference_date(&st).await?;
    let (from, to) = default_range(reference);
    let range: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT MIN(po_date), MAX(po_date) FROM purchase_orders").fetch_one(&st.db).await?;
    let cats: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM categories ORDER BY name").fetch_all(&st.db).await?;
    let vendors: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM vendors ORDER BY name").fetch_all(&st.db).await?;
    let coms: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, name, base_unit FROM commodities ORDER BY id").fetch_all(&st.db).await?;
    Ok(Json(json!({
        "reference_date": reference,
        "default_from": from,
        "default_to": to,
        "data_from": range.0,
        "data_to": range.1,
        "ai_provider": st.ai.provider_name(),
        "categories": cats.into_iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "vendors": vendors.into_iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "commodities": coms.into_iter().map(|(id, name, unit)| json!({"id": id, "name": name, "base_unit": unit})).collect::<Vec<_>>(),
    })))
}

pub async fn overview(State(st): State<AppState>, _u: CurrentUser, Query(q): Query<FilterQuery>) -> AppResult<Json<Value>> {
    let f = resolve(&st, &q).await?;
    let cur = queries::kpis(&st.db, f.from, f.to, scope(&f)).await?;
    let prev = queries::kpis(&st.db, f.prev_from, f.prev_to, scope(&f)).await?;
    let trend = queries::monthly_trend(&st.db, f.to, 12, scope(&f)).await?;
    let alerts = queries::alerts(&st.db, None, None, Some(f.to), 5).await?;
    let by_cat = queries::spend_by_category_compare(&st.db, f.from, f.to, f.prev_from, f.prev_to, scope(&f)).await?;
    Ok(Json(json!({ "filter": f, "kpi": cur, "prev_kpi": prev, "trend": trend, "alerts": alerts, "by_category": by_cat })))
}

pub async fn spend_by_category(State(st): State<AppState>, _u: CurrentUser, Query(q): Query<FilterQuery>) -> AppResult<Json<Value>> {
    let f = resolve(&st, &q).await?;
    let rows = queries::spend_by_category_compare(&st.db, f.from, f.to, f.prev_from, f.prev_to, scope(&f)).await?;
    let total = rows.iter().map(|r| r.spend).sum::<f64>();
    let prev_total = rows.iter().map(|r| r.prev_spend).sum::<f64>();
    Ok(Json(json!({ "filter": f, "categories": rows, "total": total, "prev_total": prev_total })))
}

async fn category_name(st: &AppState, id: i64) -> AppResult<String> {
    sqlx::query_scalar("SELECT name FROM categories WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Kategori tidak ditemukan".into()))
}

pub async fn spend_category(
    State(st): State<AppState>,
    _u: CurrentUser,
    Path(id): Path<i64>,
    Query(q): Query<FilterQuery>,
) -> AppResult<Json<Value>> {
    let f = resolve(&st, &q).await?;
    let name = category_name(&st, id).await?;
    let s = Scope { category: Some(id), vendor: f.vendor, commodity: None };
    let trend = queries::monthly_trend(&st.db, f.to, 12, s).await?;
    let items = queries::top_items(&st.db, f.from, f.to, s, 10).await?;
    let vendors = queries::spend_by(&st.db, queries::GroupBy::Vendor, f.from, f.to, s, Some(5)).await?;
    let mut dec = queries::decomposition(&st.db, f.from, f.to, f.prev_from, f.prev_to, s).await?;
    dec.items.truncate(15);
    Ok(Json(
        json!({ "filter": f, "category": { "id": id, "name": name }, "trend": trend, "top_items": items, "top_vendors": vendors, "decomposition": dec }),
    ))
}

/// Tabel fakta dekomposisi yang dikirim ke AI (dan dipakai untuk verifikasi angka).
pub fn decomposition_facts(name: &str, f: &Filter, d: &crate::analytics::decomposition::Decomposition) -> ToolResult {
    let r0 = |x: f64| json!(x.round());
    let mut rows = vec![
        vec![json!("Spend periode sebelumnya"), r0(d.spend0), Value::Null, Value::Null, Value::Null],
        vec![json!("Spend periode ini"), r0(d.spend1), Value::Null, Value::Null, Value::Null],
        vec![
            json!("Total perubahan"),
            r0(d.total_change),
            Value::Null,
            Value::Null,
            if d.spend0 > 0.0 { json!(((d.spend1 / d.spend0 - 1.0) * 1000.0).round() / 10.0) } else { Value::Null },
        ],
        vec![json!("Efek volume"), r0(d.volume_effect), Value::Null, Value::Null, Value::Null],
        vec![json!("Efek harga"), r0(d.price_effect), Value::Null, Value::Null, Value::Null],
        vec![json!("Efek item baru"), r0(d.new_item_effect), Value::Null, Value::Null, Value::Null],
        vec![json!("Efek item tidak dibeli lagi"), r0(d.lost_item_effect), Value::Null, Value::Null, Value::Null],
    ];
    for it in d.items.iter().take(5) {
        let pp = match (it.price0, it.price1) {
            (Some(a), Some(b)) if a > 0.0 => json!(((b / a - 1.0) * 1000.0).round() / 10.0),
            _ => Value::Null,
        };
        rows.push(vec![json!(format!("Item: {}", it.name)), r0(it.total_change), r0(it.volume_effect), r0(it.price_effect), pp]);
    }
    ToolResult {
        tool: "decomposition".into(),
        params: json!({ "category": name, "period": [f.from, f.to], "previous": [f.prev_from, f.prev_to] }),
        title: format!("Dekomposisi spend {name}: {} s/d {} vs {} s/d {}", f.from, f.to, f.prev_from, f.prev_to),
        columns: vec![
            "Komponen".into(),
            "Nilai / total perubahan (Rp)".into(),
            "Efek volume (Rp)".into(),
            "Efek harga (Rp)".into(),
            "Perubahan harga / spend (%)".into(),
        ],
        rows,
        note: None,
    }
}

pub async fn explain_category(
    State(st): State<AppState>,
    _u: CurrentUser,
    Path(id): Path<i64>,
    Query(q): Query<FilterQuery>,
) -> AppResult<Json<Value>> {
    let f = resolve(&st, &q).await?;
    let name = category_name(&st, id).await?;
    let s = Scope { category: Some(id), vendor: f.vendor, commodity: None };
    let dec = queries::decomposition(&st.db, f.from, f.to, f.prev_from, f.prev_to, s).await?;
    let facts = decomposition_facts(&name, &f, &dec);
    let allowed = allowed_numbers(&[json!(facts.rows), facts.params.clone()], std::slice::from_ref(&facts.title));
    let mut feedback: Option<String> = None;
    let mut rejected: Vec<String> = vec![];
    let mut ai_error: Option<String> = None;
    for _ in 0..2 {
        let req = AiRequest {
            purpose: AiPurpose::Explain { facts: facts.clone() },
            prompt: explain_prompt(&name, &facts, feedback.as_deref()),
            schema: explain_schema(),
        };
        match st.ai.call::<ExplainOut>(&req).await {
            Ok(out) => {
                let bad = unverified_numbers(&out.narrative, &allowed);
                if bad.is_empty() {
                    return Ok(Json(
                        json!({ "narrative": out.narrative, "verified": true, "fallback": false, "provider": st.ai.provider_name(), "facts": facts }),
                    ));
                }
                feedback = Some(bad.join(", "));
                rejected = bad;
            }
            Err(e) => {
                ai_error = Some(e.to_string());
                break;
            }
        }
    }
    let narrative = crate::ai::mock::explain_template(&facts);
    Ok(Json(json!({
        "narrative": narrative,
        "verified": true,
        "fallback": true,
        "fallback_reason": ai_error.unwrap_or_else(|| format!("narasi AI memuat angka yang tidak ada di data: {}", rejected.join(", "))),
        "provider": st.ai.provider_name(),
        "facts": facts,
    })))
}

pub async fn commodities(State(st): State<AppState>, _u: CurrentUser) -> AppResult<Json<Value>> {
    let reference = reference_date(&st).await?;
    let rows = queries::commodities_table(&st.db, reference).await?;
    let settings = Settings::load(&st.db).await?;
    let alerts = queries::alerts(&st.db, Some("price"), None, None, 100).await?;
    Ok(Json(json!({ "reference_date": reference, "commodities": rows, "alerts": alerts, "params": {
        "threshold_pct": settings.anomaly().threshold_pct, "sigma": settings.anomaly().sigma, "min_dev_pct": settings.anomaly().min_dev_pct } })))
}

#[derive(Deserialize)]
pub struct WeeksQuery {
    pub weeks: Option<i64>,
}

pub async fn commodity(
    State(st): State<AppState>,
    _u: CurrentUser,
    Path(id): Path<i64>,
    Query(q): Query<WeeksQuery>,
) -> AppResult<Json<Value>> {
    let reference = reference_date(&st).await?;
    let settings = Settings::load(&st.db).await?;
    let d = queries::commodity_detail(&st.db, id, reference, q.weeks.unwrap_or(52).clamp(4, 104), settings.anomaly()).await?;
    let alerts: Vec<_> = queries::alerts(&st.db, Some("price"), None, None, 500).await?.into_iter().filter(|a| a.ref_id == id).collect();
    Ok(Json(json!({ "detail": d, "alerts": alerts })))
}

pub async fn scorecard(State(st): State<AppState>, _u: CurrentUser, Query(q): Query<FilterQuery>) -> AppResult<Json<Value>> {
    let f = resolve(&st, &q).await?;
    let settings = Settings::load(&st.db).await?;
    let rows = queries::vendor_scorecard(&st.db, f.from, f.to, scope(&f), settings.weights()).await?;
    let problems: Vec<_> = rows.iter().filter(|r| r.label != "Baik").cloned().collect();
    Ok(Json(json!({ "filter": f, "weights": settings.weights(), "vendors": rows, "problems": problems })))
}

/// Kemungkinan split PO (bonus): vendor sama, tanggal sama, banyak PO di bawah batas approval
/// yang totalnya melewati batas.
pub async fn split_po(State(st): State<AppState>, _u: CurrentUser, Query(q): Query<FilterQuery>) -> AppResult<Json<Value>> {
    let f = resolve(&st, &q).await?;
    let p = Settings::load(&st.db).await?.split_po();
    let groups = queries::split_po_candidates(&st.db, f.from, f.to, f.vendor, p).await?;
    Ok(Json(json!({
        "filter": f,
        "params": { "approval_limit": p.approval_limit, "min_count": p.min_count },
        "groups": groups,
    })))
}

pub async fn vendor_detail(
    State(st): State<AppState>,
    _u: CurrentUser,
    Path(id): Path<i64>,
    Query(q): Query<FilterQuery>,
) -> AppResult<Json<Value>> {
    let f = resolve(&st, &q).await?;
    let name: String = sqlx::query_scalar("SELECT name FROM vendors WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Vendor tidak ditemukan".into()))?;
    let w = Settings::load(&st.db).await?.weights();
    // tren skor bulanan 12 bulan s/d bulan filter `to`
    let mut trend = Vec::new();
    for k in (0..12).rev() {
        let ms = add_months(month_start(f.to), -k);
        let me = month_end(ms).min(f.to.max(ms));
        let rows = queries::vendor_scorecard(&st.db, ms, me, Scope::default(), w).await?;
        let row = rows.into_iter().find(|r| r.vendor_id == id);
        trend.push(json!({
            "month": ms.format("%Y-%m").to_string(),
            "total": row.as_ref().and_then(|r| r.total),
            "on_time_pct": row.as_ref().and_then(|r| r.on_time_pct),
            "quality_score": row.as_ref().and_then(|r| r.quality_score),
            "price_score": row.as_ref().and_then(|r| r.price_score),
        }));
    }
    let current = queries::vendor_scorecard(&st.db, f.from, f.to, Scope { vendor: Some(id), ..Default::default() }, w).await?;
    // skor periode harus dihitung dengan harga pasar semua vendor
    let all = queries::vendor_scorecard(&st.db, f.from, f.to, Scope::default(), w).await?;
    let score = all.into_iter().find(|r| r.vendor_id == id).or_else(|| current.into_iter().next());
    let pos: Vec<(String, String, String, String, f64, String, i64, f64, String, Option<String>)> = sqlx::query_as(
        "SELECT po.po_no, po.po_date, i.name, c.name, po.qty, po.unit, po.unit_price, po.amount, po.promised_date, po.delivered_date
         FROM purchase_orders po JOIN items i ON i.id = po.item_id JOIN categories c ON c.id = i.category_id
         WHERE po.vendor_id = ? AND po.po_date BETWEEN ? AND ? ORDER BY po.po_date DESC, po.po_no DESC LIMIT 200",
    )
    .bind(id)
    .bind(f.start_iso())
    .bind(f.end_iso())
    .fetch_all(&st.db)
    .await?;
    let po_rows: Vec<Value> = pos
        .into_iter()
        .map(|(po, d, item, cat, qty, unit, price, amount, promised, delivered)| {
            let late_days = delivered.as_deref().and_then(parse_date).zip(parse_date(&promised)).map(|(dd, pp)| (dd - pp).num_days());
            json!({ "po_no": po, "po_date": d, "item": item, "category": cat, "qty": qty, "unit": unit, "unit_price": price, "amount": amount,
                    "promised_date": promised, "delivered_date": delivered, "late_days": late_days.filter(|x| *x > 0) })
        })
        .collect();
    let feedback: Vec<(String, f64, f64, f64, Option<String>)> = sqlx::query_as(
        "SELECT period, quality_score, reject_qty, total_qty, feedback FROM vendor_ratings WHERE vendor_id = ? ORDER BY period DESC LIMIT 12",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    let feedback: Vec<Value> = feedback
        .into_iter()
        .map(|(p, q, r, t, fb)| json!({ "period": p, "quality_score": q, "reject_pct": if t > 0.0 { r / t * 100.0 } else { 0.0 }, "feedback": fb }))
        .collect();
    Ok(Json(
        json!({ "filter": f, "vendor": { "id": id, "name": name }, "score": score, "trend": trend, "pos": po_rows, "feedback": feedback }),
    ))
}

#[derive(Deserialize)]
pub struct AlertQuery {
    pub kind: Option<String>,
    pub from: Option<String>,
    pub limit: Option<i64>,
}

pub async fn alerts(State(st): State<AppState>, _u: CurrentUser, Query(q): Query<AlertQuery>) -> AppResult<Json<Value>> {
    let from = q
        .from
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| parse_date(s).ok_or_else(|| AppError::BadRequest("from tidak valid".into())))
        .transpose()?;
    let kind = q.kind.as_deref().filter(|k| *k == "price" || *k == "vendor");
    let rows = queries::alerts(&st.db, kind, from, None, q.limit.unwrap_or(100).clamp(1, 1000)).await?;
    Ok(Json(json!({ "alerts": rows })))
}
