//! Agregat mingguan harga komoditas + pembuatan alert. Dihitung ulang setelah setiap
//! import, rollback, atau perubahan master/threshold.

use crate::analytics::anomaly::{detect_price_anomalies, WeekPrice};
use crate::analytics::period::{parse_period, week_label_start};
use crate::error::AppResult;
use crate::settings::Settings;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

pub fn fmt_rp(v: f64) -> String {
    let n = v.round() as i64;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    format!("Rp {}{}", if n < 0 { "-" } else { "" }, out)
}

pub fn fmt_pct(v: f64) -> String {
    format!("{:.1}%", v).replace('.', ",")
}

/// Baca seri harga mingguan satu komoditas dari tabel agregat.
pub async fn commodity_series(db: &SqlitePool, commodity_id: i64) -> AppResult<Vec<WeekPrice>> {
    let rows: Vec<(String, String, f64)> =
        sqlx::query_as("SELECT week, week_start, avg_price FROM commodity_weekly WHERE commodity_id = ? ORDER BY week_start")
            .bind(commodity_id)
            .fetch_all(db)
            .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(week, ws, price)| {
            let week_start = chrono::NaiveDate::parse_from_str(&ws, "%Y-%m-%d").ok()?;
            Some(WeekPrice { week, week_start, price })
        })
        .collect())
}

pub async fn recompute(db: &SqlitePool) -> AppResult<()> {
    let settings = Settings::load(db).await?;
    let mut tx = db.begin().await?;

    // 1) Agregat mingguan: rata-rata tertimbang qty = SUM(spend) / SUM(base_qty)
    sqlx::query("DELETE FROM commodity_weekly").execute(&mut *tx).await?;
    let rows: Vec<(i64, String, f64, f64, i64)> = sqlx::query_as(
        "SELECT i.commodity_id, po.iso_week, SUM(po.base_qty), SUM(po.amount), COUNT(*)
         FROM purchase_orders po JOIN items i ON i.id = po.item_id
         WHERE i.commodity_id IS NOT NULL
         GROUP BY i.commodity_id, po.iso_week",
    )
    .fetch_all(&mut *tx)
    .await?;
    for (cid, week, qty, spend, n) in &rows {
        if *qty <= 0.0 {
            continue;
        }
        let ws = week_label_start(week).map(|d| d.to_string()).unwrap_or_default();
        sqlx::query("INSERT INTO commodity_weekly(commodity_id, week, week_start, base_qty, spend, avg_price, po_lines) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(cid)
            .bind(week)
            .bind(ws)
            .bind(qty)
            .bind(spend)
            .bind(spend / qty)
            .bind(n)
            .execute(&mut *tx)
            .await?;
    }

    // 2) Alert harga
    sqlx::query("DELETE FROM alerts").execute(&mut *tx).await?;
    let commodities: Vec<(i64, String, String)> = sqlx::query_as("SELECT id, name, base_unit FROM commodities").fetch_all(&mut *tx).await?;
    let params = settings.anomaly();
    let mut by_com: BTreeMap<i64, Vec<WeekPrice>> = BTreeMap::new();
    for (cid, week, qty, spend, _) in rows {
        if qty <= 0.0 {
            continue;
        }
        if let Some(ws) = week_label_start(&week) {
            by_com.entry(cid).or_default().push(WeekPrice { week, week_start: ws, price: spend / qty });
        }
    }
    for (cid, name, unit) in commodities {
        let Some(series) = by_com.get_mut(&cid) else { continue };
        series.sort_by_key(|w| w.week_start);
        for a in detect_price_anomalies(series, params) {
            let mut parts = Vec::new();
            if let (true, Some(c)) = (a.by_threshold, a.change_pct) {
                parts.push(format!("naik {} vs minggu lalu", fmt_pct(c)));
            }
            if let (true, Some(m), Some(s)) = (a.by_sigma, a.mean12, a.std12) {
                parts.push(format!("di atas rata-rata 12 minggu + {}σ ({})", params.sigma, fmt_rp(m + params.sigma * s)));
            }
            let value = a.change_pct.unwrap_or_else(|| a.mean12.map(|m| (a.price / m - 1.0) * 100.0).unwrap_or(0.0));
            let msg = format!("Harga {} minggu {}: {}/{} — {}", name, a.week, fmt_rp(a.price), unit, parts.join("; "));
            sqlx::query("INSERT INTO alerts(kind, ref_id, week, period_start, message, value) VALUES ('price', ?, ?, ?, ?, ?)")
                .bind(cid)
                .bind(&a.week)
                .bind(a.week_start.to_string())
                .bind(msg)
                .bind(value)
                .execute(&mut *tx)
                .await?;
        }
    }

    // 3) Alert vendor bulanan: ketepatan kirim rendah atau reject tinggi
    let on_time_min = settings.num("vendor_alert_on_time_pct", 70.0);
    let reject_max = settings.num("vendor_alert_reject_pct", 5.0);
    let late: Vec<(i64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT v.id, v.name, substr(po.delivered_date, 1, 7) AS m,
                COUNT(DISTINCT po.po_no), COUNT(DISTINCT CASE WHEN po.delivered_date <= po.promised_date THEN po.po_no END)
         FROM purchase_orders po JOIN vendors v ON v.id = po.vendor_id
         WHERE po.delivered_date IS NOT NULL
         GROUP BY v.id, m HAVING COUNT(DISTINCT po.po_no) >= 5",
    )
    .fetch_all(&mut *tx)
    .await?;
    for (vid, vname, month, total, on_time) in late {
        let pct = on_time as f64 / total as f64 * 100.0;
        if pct < on_time_min {
            let start = parse_period(&month).map(|p| p.0.to_string()).unwrap_or_default();
            let msg = format!("{vname}: ketepatan kirim {month} hanya {} ({on_time} dari {total} PO tepat waktu)", fmt_pct(pct));
            sqlx::query("INSERT INTO alerts(kind, ref_id, week, period_start, message, value) VALUES ('vendor', ?, ?, ?, ?, ?)")
                .bind(vid)
                .bind(&month)
                .bind(start)
                .bind(msg)
                .bind(pct)
                .execute(&mut *tx)
                .await?;
        }
    }
    let rejects: Vec<(i64, String, String, f64, f64, f64)> = sqlx::query_as(
        "SELECT v.id, v.name, r.period, r.reject_qty, r.total_qty, r.quality_score
         FROM vendor_ratings r JOIN vendors v ON v.id = r.vendor_id WHERE r.total_qty > 0",
    )
    .fetch_all(&mut *tx)
    .await?;
    for (vid, vname, period, rej, tot, q) in rejects {
        let pct = rej / tot * 100.0;
        if pct > reject_max {
            let start = parse_period(&period).map(|p| p.0.to_string()).unwrap_or_default();
            let msg = format!("{vname}: reject {period} {} dari total diterima, skor kualitas {:.1}", fmt_pct(pct), q);
            sqlx::query("INSERT INTO alerts(kind, ref_id, week, period_start, message, value) VALUES ('vendor', ?, ?, ?, ?, ?)")
                .bind(vid)
                .bind(&period)
                .bind(start)
                .bind(msg)
                .bind(pct)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// Hitung ulang base_qty & base_unit_price semua PO (setelah konversi satuan / mapping berubah).
pub async fn renormalize(db: &SqlitePool) -> AppResult<usize> {
    let master = crate::import::load_master(db).await?;
    let rows: Vec<(i64, i64, String, f64, i64)> =
        sqlx::query_as("SELECT id, item_id, unit, qty, unit_price FROM purchase_orders").fetch_all(db).await?;
    let base_by_item: std::collections::HashMap<i64, String> = master.items.values().map(|i| (i.id, i.base_unit.clone())).collect();
    let mut tx = db.begin().await?;
    let mut skipped = 0;
    for (id, item_id, unit, qty, price) in rows {
        let Some(base) = base_by_item.get(&item_id) else { continue };
        match master.units.factor(Some(item_id), base, &unit) {
            Ok(f) => {
                sqlx::query("UPDATE purchase_orders SET base_qty = ?, base_unit_price = ? WHERE id = ?")
                    .bind(qty * f)
                    .bind(price as f64 / f)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
            Err(_) => skipped += 1,
        }
    }
    tx.commit().await?;
    recompute(db).await?;
    Ok(skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_rupiah_dan_persen() {
        assert_eq!(fmt_rp(14840.4), "Rp 14.840");
        assert_eq!(fmt_rp(1234567.0), "Rp 1.234.567");
        assert_eq!(fmt_rp(999.0), "Rp 999");
        assert_eq!(fmt_pct(16.04), "16,0%");
    }
}
