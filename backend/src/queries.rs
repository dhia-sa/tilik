//! Query analitik yang dipakai bersama oleh halaman dashboard, export Excel, dan tool Tanya Data.
//! Semua angka di UI dan jawaban AI berasal dari fungsi-fungsi ini, sehingga konsisten.

use crate::aggregates::commodity_series;
use crate::analytics::anomaly::{evaluate_week, AnomalyParams};
use crate::analytics::decomposition::{decompose, Decomposition, ItemPeriods};
use crate::analytics::forecast::{forecast_weeks, Forecast};
use crate::analytics::period::{add_months, iso_week_label, month_end, month_start};
use crate::analytics::scorecard::{price_diff_by_vendor, score, VendorInputs, Weights};
use crate::analytics::split_po::{detect_split_po, PoTotal, SplitGroup, SplitParams};
use crate::error::{AppError, AppResult};
use chrono::{Duration, NaiveDate};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;

/// Klausa filter standar untuk tabel purchase_orders (alias po) JOIN items (alias i).
const PO_FILTER: &str = "po.po_date BETWEEN ? AND ?
    AND (? IS NULL OR i.category_id = ?)
    AND (? IS NULL OR po.vendor_id = ?)
    AND (? IS NULL OR i.commodity_id = ?)";

#[derive(Debug, Clone, Copy, Default)]
pub struct Scope {
    pub category: Option<i64>,
    pub vendor: Option<i64>,
    pub commodity: Option<i64>,
}

macro_rules! bind_filter {
    ($q:expr, $from:expr, $to:expr, $s:expr) => {
        $q.bind($from.to_string())
            .bind($to.to_string())
            .bind($s.category)
            .bind($s.category)
            .bind($s.vendor)
            .bind($s.vendor)
            .bind($s.commodity)
            .bind($s.commodity)
    };
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Kpi {
    pub total_spend: f64,
    pub po_count: i64,
    pub active_vendors: i64,
    pub on_time_pct: Option<f64>,
    pub delivered_po: i64,
}

pub async fn kpis(db: &SqlitePool, from: NaiveDate, to: NaiveDate, s: Scope) -> AppResult<Kpi> {
    let sql = format!(
        "SELECT TOTAL(po.amount), COUNT(DISTINCT po.po_no), COUNT(DISTINCT po.vendor_id),
                COUNT(DISTINCT CASE WHEN po.delivered_date IS NOT NULL THEN po.po_no END),
                COUNT(DISTINCT CASE WHEN po.delivered_date IS NOT NULL AND po.delivered_date <= po.promised_date THEN po.po_no END)
         FROM purchase_orders po JOIN items i ON i.id = po.item_id WHERE {PO_FILTER}"
    );
    let (spend, pos, vendors, delivered, on_time): (f64, i64, i64, i64, i64) =
        bind_filter!(sqlx::query_as(&sql), from, to, s).fetch_one(db).await?;
    Ok(Kpi {
        total_spend: spend,
        po_count: pos,
        active_vendors: vendors,
        on_time_pct: (delivered > 0).then(|| on_time as f64 / delivered as f64 * 100.0),
        delivered_po: delivered,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct MonthPoint {
    pub month: String,
    pub spend: f64,
    pub po_count: i64,
}

/// Tren spend bulanan `months` bulan yang berakhir di bulan `end`.
pub async fn monthly_trend(db: &SqlitePool, end: NaiveDate, months: i32, s: Scope) -> AppResult<Vec<MonthPoint>> {
    let first = add_months(month_start(end), -(months - 1));
    let last = month_end(end);
    let sql = format!(
        "SELECT substr(po.po_date, 1, 7) AS m, TOTAL(po.amount), COUNT(DISTINCT po.po_no)
         FROM purchase_orders po JOIN items i ON i.id = po.item_id WHERE {PO_FILTER} GROUP BY m"
    );
    let rows: Vec<(String, f64, i64)> = bind_filter!(sqlx::query_as(&sql), first, last, s).fetch_all(db).await?;
    let map: HashMap<String, (f64, i64)> = rows.into_iter().map(|(m, a, c)| (m, (a, c))).collect();
    Ok((0..months)
        .map(|k| {
            let m = add_months(first, k).format("%Y-%m").to_string();
            let (spend, po_count) = map.get(&m).copied().unwrap_or((0.0, 0));
            MonthPoint { month: m, spend, po_count }
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    Category,
    Vendor,
    Item,
    Month,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpendRow {
    pub id: Option<i64>,
    pub name: String,
    pub spend: f64,
    pub po_count: i64,
    pub share_pct: f64,
}

pub async fn spend_by(
    db: &SqlitePool,
    group: GroupBy,
    from: NaiveDate,
    to: NaiveDate,
    s: Scope,
    limit: Option<i64>,
) -> AppResult<Vec<SpendRow>> {
    let (key, name, join, order) = match group {
        GroupBy::Category => ("c.id", "c.name", "JOIN categories c ON c.id = i.category_id", "2 DESC"),
        GroupBy::Vendor => ("v.id", "v.name", "JOIN vendors v ON v.id = po.vendor_id", "2 DESC"),
        GroupBy::Item => ("i.id", "i.name", "", "2 DESC"),
        GroupBy::Month => ("NULL", "substr(po.po_date, 1, 7)", "", "1 ASC"),
    };
    let sql = format!(
        "SELECT {name} AS n, TOTAL(po.amount) AS spend, COUNT(DISTINCT po.po_no), {key}
         FROM purchase_orders po JOIN items i ON i.id = po.item_id {join}
         WHERE {PO_FILTER} GROUP BY n ORDER BY {order} LIMIT ?"
    );
    let rows: Vec<(String, f64, i64, Option<i64>)> =
        bind_filter!(sqlx::query_as(&sql), from, to, s).bind(limit.unwrap_or(1000)).fetch_all(db).await?;
    let total = kpis(db, from, to, s).await?.total_spend;
    Ok(rows
        .into_iter()
        .map(|(name, spend, po_count, id)| SpendRow {
            id,
            name,
            spend,
            po_count,
            share_pct: if total > 0.0 { spend / total * 100.0 } else { 0.0 },
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct CategorySpend {
    pub id: i64,
    pub name: String,
    pub spend: f64,
    pub prev_spend: f64,
    pub change_pct: Option<f64>,
    pub share_pct: f64,
}

pub async fn spend_by_category_compare(
    db: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
    pf: NaiveDate,
    pt: NaiveDate,
    s: Scope,
) -> AppResult<Vec<CategorySpend>> {
    let cur = spend_by(db, GroupBy::Category, from, to, Scope { category: None, ..s }, None).await?;
    let prev = spend_by(db, GroupBy::Category, pf, pt, Scope { category: None, ..s }, None).await?;
    let prev_map: HashMap<Option<i64>, f64> = prev.iter().map(|r| (r.id, r.spend)).collect();
    let cats: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM categories ORDER BY id").fetch_all(db).await?;
    let cur_map: HashMap<Option<i64>, &SpendRow> = cur.iter().map(|r| (r.id, r)).collect();
    let mut out: Vec<CategorySpend> = cats
        .into_iter()
        .map(|(id, name)| {
            let spend = cur_map.get(&Some(id)).map(|r| r.spend).unwrap_or(0.0);
            let share = cur_map.get(&Some(id)).map(|r| r.share_pct).unwrap_or(0.0);
            let prev_spend = prev_map.get(&Some(id)).copied().unwrap_or(0.0);
            CategorySpend {
                id,
                name,
                spend,
                prev_spend,
                change_pct: (prev_spend > 0.0).then(|| (spend / prev_spend - 1.0) * 100.0),
                share_pct: share,
            }
        })
        .collect();
    out.sort_by(|a, b| b.spend.total_cmp(&a.spend));
    Ok(out)
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemSpend {
    pub item_id: i64,
    pub item: String,
    pub category: String,
    pub spend: f64,
    pub base_qty: f64,
    pub base_unit: String,
    pub avg_price: f64,
    pub po_count: i64,
}

pub async fn top_items(db: &SqlitePool, from: NaiveDate, to: NaiveDate, s: Scope, limit: i64) -> AppResult<Vec<ItemSpend>> {
    let sql = format!(
        "SELECT i.id, i.name, c.name, TOTAL(po.amount) AS spend, TOTAL(po.base_qty), i.base_unit, COUNT(DISTINCT po.po_no)
         FROM purchase_orders po JOIN items i ON i.id = po.item_id JOIN categories c ON c.id = i.category_id
         WHERE {PO_FILTER} GROUP BY i.id ORDER BY spend DESC LIMIT ?"
    );
    let rows: Vec<(i64, String, String, f64, f64, String, i64)> =
        bind_filter!(sqlx::query_as(&sql), from, to, s).bind(limit).fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(|(item_id, item, category, spend, base_qty, base_unit, po_count)| ItemSpend {
            item_id,
            item,
            category,
            spend,
            base_qty,
            base_unit,
            avg_price: if base_qty > 0.0 { spend / base_qty } else { 0.0 },
            po_count,
        })
        .collect())
}

pub async fn decomposition(
    db: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
    pf: NaiveDate,
    pt: NaiveDate,
    s: Scope,
) -> AppResult<Decomposition> {
    let sql = format!(
        "SELECT i.id, i.name, TOTAL(po.base_qty), TOTAL(po.amount)
         FROM purchase_orders po JOIN items i ON i.id = po.item_id WHERE {PO_FILTER} GROUP BY i.id"
    );
    let cur: Vec<(i64, String, f64, f64)> = bind_filter!(sqlx::query_as(&sql), from, to, s).fetch_all(db).await?;
    let prev: Vec<(i64, String, f64, f64)> = bind_filter!(sqlx::query_as(&sql), pf, pt, s).fetch_all(db).await?;
    let mut map: HashMap<i64, ItemPeriods> = HashMap::new();
    for (id, name, q, sp) in prev {
        map.insert(id, ItemPeriods { item_id: id, name, qty0: q, spend0: sp, qty1: 0.0, spend1: 0.0 });
    }
    for (id, name, q, sp) in cur {
        let e = map.entry(id).or_insert(ItemPeriods { item_id: id, name, qty0: 0.0, spend0: 0.0, qty1: 0.0, spend1: 0.0 });
        e.qty1 = q;
        e.spend1 = sp;
    }
    let rows: Vec<ItemPeriods> = map.into_values().collect();
    Ok(decompose(&rows))
}

// ---------------------------------------------------------------- Komoditas

#[derive(Debug, Clone, Serialize)]
pub struct WeekPoint {
    pub week: String,
    pub week_start: String,
    pub price: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommodityRow {
    pub id: i64,
    pub name: String,
    pub base_unit: String,
    pub this_week: String,
    pub price_this_week: Option<f64>,
    pub price_last_week: Option<f64>,
    pub change_pct: Option<f64>,
    pub qty_this_week: f64,
    pub sparkline: Vec<WeekPoint>,
    pub alert: bool,
    pub alert_message: Option<String>,
}

pub fn weeks_ending(reference: NaiveDate, n: i64) -> Vec<(String, NaiveDate)> {
    let last = crate::analytics::period::week_start(reference);
    (0..n)
        .rev()
        .map(|k| {
            let ws = last - Duration::days(7 * k);
            (iso_week_label(ws), ws)
        })
        .collect()
}

pub async fn commodities_table(db: &SqlitePool, reference: NaiveDate) -> AppResult<Vec<CommodityRow>> {
    let weeks = weeks_ending(reference, 12);
    let this_week = weeks.last().map(|w| w.0.clone()).unwrap_or_default();
    let last_week = weeks.iter().rev().nth(1).map(|w| w.0.clone()).unwrap_or_default();
    let coms: Vec<(i64, String, String)> = sqlx::query_as("SELECT id, name, base_unit FROM commodities ORDER BY id").fetch_all(db).await?;
    let first = weeks.first().map(|w| w.1.to_string()).unwrap_or_default();
    let rows: Vec<(i64, String, f64, f64)> =
        sqlx::query_as("SELECT commodity_id, week, avg_price, base_qty FROM commodity_weekly WHERE week_start >= ?")
            .bind(first)
            .fetch_all(db)
            .await?;
    let mut map: HashMap<(i64, String), (f64, f64)> = HashMap::new();
    for (c, w, p, q) in rows {
        map.insert((c, w), (p, q));
    }
    let alerts: Vec<(i64, String)> =
        sqlx::query_as("SELECT ref_id, message FROM alerts WHERE kind = 'price' AND week = ?").bind(&this_week).fetch_all(db).await?;
    let alert_map: HashMap<i64, String> = alerts.into_iter().collect();
    Ok(coms
        .into_iter()
        .map(|(id, name, unit)| {
            let cur = map.get(&(id, this_week.clone())).copied();
            let prev = map.get(&(id, last_week.clone())).map(|x| x.0);
            let pc = cur.map(|x| x.0);
            CommodityRow {
                id,
                name,
                base_unit: unit,
                this_week: this_week.clone(),
                price_this_week: pc,
                price_last_week: prev,
                change_pct: match (pc, prev) {
                    (Some(a), Some(b)) if b > 0.0 => Some((a / b - 1.0) * 100.0),
                    _ => None,
                },
                qty_this_week: cur.map(|x| x.1).unwrap_or(0.0),
                sparkline: weeks
                    .iter()
                    .map(|(w, ws)| WeekPoint { week: w.clone(), week_start: ws.to_string(), price: map.get(&(id, w.clone())).map(|x| x.0) })
                    .collect(),
                alert: alert_map.contains_key(&id),
                alert_message: alert_map.get(&id).cloned(),
            }
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct WeekDetail {
    pub week: String,
    pub week_start: String,
    pub price: Option<f64>,
    pub qty: f64,
    pub change_pct: Option<f64>,
    pub mean12: Option<f64>,
    pub upper_band: Option<f64>,
    pub alert: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VendorPrice {
    pub vendor_id: i64,
    pub vendor: String,
    pub avg_price_4w: Option<f64>,
    pub avg_price_52w: f64,
    pub qty_52w: f64,
    pub diff_vs_market_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommodityDetail {
    pub id: i64,
    pub name: String,
    pub base_unit: String,
    pub weeks: Vec<WeekDetail>,
    pub vendors: Vec<VendorPrice>,
    pub vendor_weekly: Vec<serde_json::Value>,
    /// Proyeksi 4 minggu setelah minggu data terakhir (None kalau data < 4 minggu).
    pub forecast: Option<Forecast>,
}

/// Jumlah minggu data yang dipakai regresi proyeksi, dan jumlah minggu yang diproyeksikan.
pub const FORECAST_WINDOW: i64 = 12;
pub const FORECAST_HORIZON: i64 = 4;

pub async fn commodity_detail(
    db: &SqlitePool,
    id: i64,
    reference: NaiveDate,
    n_weeks: i64,
    params: AnomalyParams,
) -> AppResult<CommodityDetail> {
    let (name, unit): (String, String) = sqlx::query_as("SELECT name, base_unit FROM commodities WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Komoditas tidak ditemukan".into()))?;
    let series = commodity_series(db, id).await?;
    let ref_week = crate::analytics::period::week_start(reference);
    let upto: Vec<_> = series.iter().filter(|w| w.week_start <= ref_week).cloned().collect();
    let forecast = forecast_weeks(&upto, FORECAST_WINDOW, FORECAST_HORIZON);
    let qty: HashMap<String, f64> =
        sqlx::query_as::<_, (String, f64)>("SELECT week, base_qty FROM commodity_weekly WHERE commodity_id = ?")
            .bind(id)
            .fetch_all(db)
            .await?
            .into_iter()
            .collect();
    let idx: HashMap<String, usize> = series.iter().enumerate().map(|(i, w)| (w.week.clone(), i)).collect();
    let weeks: Vec<WeekDetail> = weeks_ending(reference, n_weeks)
        .into_iter()
        .map(|(w, ws)| match idx.get(&w) {
            Some(i) => {
                let a = evaluate_week(&series, *i, params);
                WeekDetail {
                    week: w.clone(),
                    week_start: ws.to_string(),
                    price: Some(a.price),
                    qty: qty.get(&w).copied().unwrap_or(0.0),
                    change_pct: a.change_pct,
                    mean12: a.mean12,
                    upper_band: match (a.mean12, a.std12) {
                        (Some(m), Some(s)) => Some(m + params.sigma * s),
                        _ => None,
                    },
                    alert: a.by_threshold || a.by_sigma,
                }
            }
            None => WeekDetail {
                week: w,
                week_start: ws.to_string(),
                price: None,
                qty: 0.0,
                change_pct: None,
                mean12: None,
                upper_band: None,
                alert: false,
            },
        })
        .collect();
    let start52 = weeks.first().map(|w| w.week_start.clone()).unwrap_or_default();
    let start4 = weeks.iter().rev().nth(3).map(|w| w.week_start.clone()).unwrap_or_default();
    let vrows: Vec<(i64, String, f64, f64, f64, f64)> = sqlx::query_as(
        "SELECT v.id, v.name, SUM(po.amount), SUM(po.base_qty),
                TOTAL(CASE WHEN po.po_date >= ? THEN po.amount END), TOTAL(CASE WHEN po.po_date >= ? THEN po.base_qty END)
         FROM purchase_orders po JOIN items i ON i.id = po.item_id JOIN vendors v ON v.id = po.vendor_id
         WHERE i.commodity_id = ? AND po.po_date BETWEEN ? AND ?
         GROUP BY v.id ORDER BY SUM(po.base_qty) DESC",
    )
    .bind(&start4)
    .bind(&start4)
    .bind(id)
    .bind(&start52)
    .bind(reference.to_string())
    .fetch_all(db)
    .await?;
    let (ts, tq) = vrows.iter().fold((0.0, 0.0), |(s, q), r| (s + r.2, q + r.3));
    let market = if tq > 0.0 { ts / tq } else { 0.0 };
    let vendors = vrows
        .iter()
        .map(|(vid, vn, sp, q, sp4, q4)| VendorPrice {
            vendor_id: *vid,
            vendor: vn.clone(),
            avg_price_4w: (*q4 > 0.0).then(|| sp4 / q4),
            avg_price_52w: if *q > 0.0 { sp / q } else { 0.0 },
            qty_52w: *q,
            diff_vs_market_pct: if *q > 0.0 && market > 0.0 { (sp / q / market - 1.0) * 100.0 } else { 0.0 },
        })
        .collect();
    // harga per vendor per minggu (untuk grafik)
    let vw: Vec<(String, String, f64, f64)> = sqlx::query_as(
        "SELECT po.iso_week, v.name, SUM(po.amount), SUM(po.base_qty)
         FROM purchase_orders po JOIN items i ON i.id = po.item_id JOIN vendors v ON v.id = po.vendor_id
         WHERE i.commodity_id = ? AND po.po_date BETWEEN ? AND ? GROUP BY po.iso_week, v.name",
    )
    .bind(id)
    .bind(&start52)
    .bind(reference.to_string())
    .fetch_all(db)
    .await?;
    let mut by_week: HashMap<String, serde_json::Map<String, serde_json::Value>> = HashMap::new();
    for (w, v, s, q) in vw {
        if q > 0.0 {
            by_week.entry(w).or_default().insert(v, serde_json::json!((s / q).round()));
        }
    }
    let vendor_weekly = weeks
        .iter()
        .map(|w| {
            let mut m = by_week.remove(&w.week).unwrap_or_default();
            m.insert("week".into(), serde_json::json!(w.week));
            serde_json::Value::Object(m)
        })
        .collect();
    Ok(CommodityDetail { id, name, base_unit: unit, weeks, vendors, vendor_weekly, forecast })
}

/// Cari id komoditas dari nama (mis. "gula" -> Gula Pasir).
pub async fn find_commodity(db: &SqlitePool, q: &str) -> AppResult<Option<(i64, String, String)>> {
    let all: Vec<(i64, String, String)> = sqlx::query_as("SELECT id, name, base_unit FROM commodities").fetch_all(db).await?;
    Ok(fuzzy_pick(all, q, |r| r.1.clone()))
}

pub async fn find_category(db: &SqlitePool, q: &str) -> AppResult<Option<(i64, String)>> {
    let all: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM categories").fetch_all(db).await?;
    Ok(fuzzy_pick(all, q, |r| r.1.clone()))
}

pub async fn find_item(db: &SqlitePool, q: &str) -> AppResult<Option<(i64, String, String)>> {
    let all: Vec<(i64, String, String)> = sqlx::query_as("SELECT id, name, base_unit FROM items").fetch_all(db).await?;
    Ok(fuzzy_pick(all, q, |r| r.1.clone()))
}

pub async fn find_vendor(db: &SqlitePool, q: &str) -> AppResult<Option<(i64, String)>> {
    let all: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM vendors").fetch_all(db).await?;
    Ok(fuzzy_pick(all, q, |r| r.1.clone()))
}

/// Cocokkan nama: sama persis > diawali > mengandung semua kata.
pub fn fuzzy_pick<T>(items: Vec<T>, q: &str, name: impl Fn(&T) -> String) -> Option<T> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let words: Vec<&str> = q.split_whitespace().collect();
    let mut best: Option<(i32, usize, T)> = None;
    for it in items {
        let n = name(&it).to_lowercase();
        let score = if n == q {
            3
        } else if n.starts_with(&q) {
            2
        } else if words.iter().all(|w| n.contains(w)) {
            1
        } else {
            0
        };
        if score > 0 && best.as_ref().map(|b| score > b.0 || (score == b.0 && n.len() < b.1)).unwrap_or(true) {
            best = Some((score, n.len(), it));
        }
    }
    best.map(|b| b.2)
}

// ---------------------------------------------------------------- Vendor

#[derive(Debug, Clone, Serialize)]
pub struct VendorScoreRow {
    pub vendor_id: i64,
    pub vendor: String,
    pub spend: f64,
    pub po_count: i64,
    pub delivered: i64,
    pub on_time: i64,
    pub late: i64,
    pub on_time_pct: Option<f64>,
    pub avg_quality: Option<f64>,
    pub reject_pct: Option<f64>,
    pub quality_score: Option<f64>,
    pub price_diff_pct: Option<f64>,
    pub price_score: Option<f64>,
    pub total: Option<f64>,
    pub label: String,
}

pub async fn vendor_scorecard(db: &SqlitePool, from: NaiveDate, to: NaiveDate, s: Scope, w: Weights) -> AppResult<Vec<VendorScoreRow>> {
    let sql = format!(
        "SELECT v.id, v.name, TOTAL(po.amount), COUNT(DISTINCT po.po_no),
                COUNT(DISTINCT CASE WHEN po.delivered_date IS NOT NULL THEN po.po_no END),
                COUNT(DISTINCT CASE WHEN po.delivered_date IS NOT NULL AND po.delivered_date <= po.promised_date THEN po.po_no END)
         FROM purchase_orders po JOIN items i ON i.id = po.item_id JOIN vendors v ON v.id = po.vendor_id
         WHERE {PO_FILTER} GROUP BY v.id"
    );
    let base: Vec<(i64, String, f64, i64, i64, i64)> = bind_filter!(sqlx::query_as(&sql), from, to, s).fetch_all(db).await?;
    let ratings: Vec<(i64, f64, f64, f64)> = sqlx::query_as(
        "SELECT vendor_id, AVG(quality_score), SUM(reject_qty), SUM(total_qty) FROM vendor_ratings
         WHERE period BETWEEN ? AND ? GROUP BY vendor_id",
    )
    .bind(from.format("%Y-%m").to_string())
    .bind(to.format("%Y-%m").to_string())
    .fetch_all(db)
    .await?;
    let rmap: HashMap<i64, (f64, f64)> =
        ratings.into_iter().map(|(v, q, r, t)| (v, (q, if t > 0.0 { r / t * 100.0 } else { 0.0 }))).collect();
    // harga pasar dihitung dari semua vendor (tanpa filter vendor) pada periode & kategori yang sama
    let price_sql = format!(
        "SELECT po.vendor_id, po.item_id, SUM(po.amount), SUM(po.base_qty)
         FROM purchase_orders po JOIN items i ON i.id = po.item_id WHERE {PO_FILTER} GROUP BY po.vendor_id, po.item_id"
    );
    let market_scope = Scope { vendor: None, ..s };
    let prow: Vec<(i64, i64, f64, f64)> = bind_filter!(sqlx::query_as(&price_sql), from, to, market_scope).fetch_all(db).await?;
    let diffs = price_diff_by_vendor(&prow);
    let mut out: Vec<VendorScoreRow> = base
        .into_iter()
        .map(|(vid, name, spend, pos, delivered, on_time)| {
            let r = rmap.get(&vid).copied();
            let inp = VendorInputs {
                delivered,
                on_time,
                avg_quality: r.map(|x| x.0),
                reject_pct: r.map(|x| x.1),
                price_diff_pct: diffs.get(&vid).copied(),
            };
            let sc = score(&inp, w);
            VendorScoreRow {
                vendor_id: vid,
                vendor: name,
                spend,
                po_count: pos,
                delivered,
                on_time,
                late: delivered - on_time,
                on_time_pct: sc.on_time_pct,
                avg_quality: r.map(|x| x.0),
                reject_pct: r.map(|x| x.1),
                quality_score: sc.quality_score,
                price_diff_pct: sc.price_diff_pct,
                price_score: sc.price_score,
                total: sc.total,
                label: sc.label,
            }
        })
        .collect();
    out.sort_by(|a, b| b.total.unwrap_or(-1.0).total_cmp(&a.total.unwrap_or(-1.0)));
    Ok(out)
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertRow {
    pub id: i64,
    pub kind: String,
    pub ref_id: i64,
    pub ref_name: String,
    pub week: String,
    pub period_start: String,
    pub message: String,
    pub value: f64,
}

pub async fn alerts(
    db: &SqlitePool,
    kind: Option<&str>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    limit: i64,
) -> AppResult<Vec<AlertRow>> {
    let rows: Vec<(i64, String, i64, Option<String>, String, String, String, f64)> = sqlx::query_as(
        "SELECT a.id, a.kind, a.ref_id,
                CASE a.kind WHEN 'price' THEN (SELECT name FROM commodities WHERE id = a.ref_id) ELSE (SELECT name FROM vendors WHERE id = a.ref_id) END,
                a.week, a.period_start, a.message, a.value
         FROM alerts a
         WHERE (? IS NULL OR a.kind = ?) AND (? IS NULL OR a.period_start >= ?) AND (? IS NULL OR a.period_start <= ?)
         ORDER BY a.period_start DESC, a.kind, a.id LIMIT ?",
    )
    .bind(kind)
    .bind(kind)
    .bind(from.map(|d| d.to_string()))
    .bind(from.map(|d| d.to_string()))
    .bind(to.map(|d| d.to_string()))
    .bind(to.map(|d| d.to_string()))
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, kind, ref_id, ref_name, week, period_start, message, value)| AlertRow {
            id,
            kind,
            ref_id,
            ref_name: ref_name.unwrap_or_default(),
            week,
            period_start,
            message,
            value,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct PriceChange {
    pub name: String,
    pub base_unit: String,
    pub period_a: String,
    pub period_b: String,
    pub price_a: Option<f64>,
    pub price_b: Option<f64>,
    pub change_pct: Option<f64>,
    pub qty_a: f64,
    pub qty_b: f64,
}

/// Harga rata-rata tertimbang item/komoditas di dua periode.
pub async fn price_change(
    db: &SqlitePool,
    item: Option<i64>,
    commodity: Option<i64>,
    name: &str,
    unit: &str,
    a: (NaiveDate, NaiveDate, String),
    b: (NaiveDate, NaiveDate, String),
) -> AppResult<PriceChange> {
    let q = |from: NaiveDate, to: NaiveDate| {
        sqlx::query_as::<_, (f64, f64)>(
            "SELECT TOTAL(po.amount), TOTAL(po.base_qty) FROM purchase_orders po JOIN items i ON i.id = po.item_id
             WHERE po.po_date BETWEEN ? AND ? AND (? IS NULL OR i.id = ?) AND (? IS NULL OR i.commodity_id = ?)",
        )
        .bind(from.to_string())
        .bind(to.to_string())
        .bind(item)
        .bind(item)
        .bind(commodity)
        .bind(commodity)
    };
    let (sa, qa) = q(a.0, a.1).fetch_one(db).await?;
    let (sb, qb) = q(b.0, b.1).fetch_one(db).await?;
    let pa = (qa > 0.0).then(|| sa / qa);
    let pb = (qb > 0.0).then(|| sb / qb);
    Ok(PriceChange {
        name: name.to_string(),
        base_unit: unit.to_string(),
        period_a: a.2,
        period_b: b.2,
        price_a: pa,
        price_b: pb,
        change_pct: match (pa, pb) {
            (Some(x), Some(y)) if x > 0.0 => Some((y / x - 1.0) * 100.0),
            _ => None,
        },
        qty_a: qa,
        qty_b: qb,
    })
}

// ---------------------------------------------------------------- Split PO

/// Kemungkinan split PO dalam periode. Filter kategori sengaja tidak dipakai: batas approval
/// berlaku untuk nilai PO utuh, bukan sebagian baris item.
pub async fn split_po_candidates(
    db: &SqlitePool,
    from: NaiveDate,
    to: NaiveDate,
    vendor: Option<i64>,
    p: SplitParams,
) -> AppResult<Vec<SplitGroup>> {
    let rows: Vec<(String, i64, String, String, f64, String)> = sqlx::query_as(
        "SELECT po.po_no, v.id, v.name, po.po_date, po.amount, i.name
         FROM purchase_orders po JOIN vendors v ON v.id = po.vendor_id JOIN items i ON i.id = po.item_id
         WHERE po.po_date BETWEEN ? AND ? AND (? IS NULL OR po.vendor_id = ?)
         ORDER BY po.po_no, i.name",
    )
    .bind(from.to_string())
    .bind(to.to_string())
    .bind(vendor)
    .bind(vendor)
    .fetch_all(db)
    .await?;
    let mut pos: HashMap<(String, i64, String), PoTotal> = HashMap::new();
    for (po_no, vid, vname, date, amount, item) in rows {
        let Ok(po_date) = NaiveDate::parse_from_str(&date, "%Y-%m-%d") else { continue };
        let e = pos.entry((po_no.clone(), vid, date)).or_insert_with(|| PoTotal {
            po_no,
            vendor_id: vid,
            vendor: vname,
            po_date,
            amount: 0.0,
            items: Vec::new(),
        });
        e.amount += amount;
        if !e.items.contains(&item) {
            e.items.push(item);
        }
    }
    Ok(detect_split_po(pos.into_values().collect(), p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cocokkan_nama() {
        let items = vec![(1, "Gula Pasir".to_string()), (2, "Tepung Terigu".to_string()), (3, "Gula Pasir Kristal".to_string())];
        assert_eq!(fuzzy_pick(items.clone(), "gula", |r| r.1.clone()).map(|r| r.0), Some(1));
        assert_eq!(fuzzy_pick(items.clone(), "tepung", |r| r.1.clone()).map(|r| r.0), Some(2));
        assert_eq!(fuzzy_pick(items.clone(), "emas", |r| r.1.clone()), None);
    }

    #[test]
    fn minggu_berakhir_di_tanggal_acuan() {
        let w = weeks_ending(NaiveDate::from_ymd_opt(2026, 11, 15).expect("valid"), 3);
        assert_eq!(w.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["2026-W44", "2026-W45", "2026-W46"]);
    }
}
