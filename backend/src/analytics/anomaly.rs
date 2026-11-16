//! Deteksi anomali harga komoditas mingguan.
//!
//! Alert dibuat kalau salah satu terpenuhi:
//!  1. kenaikan minggu ini vs minggu lalu (minggu ISO berurutan) > threshold (default 5%)
//!  2. harga > rata-rata 12 minggu sebelumnya + k * standar deviasi (default k = 2)
//!     DAN minimal `min_dev_pct` di atas rata-rata (default 2%) agar fluktuasi kecil
//!     yang kebetulan > 2 sigma tidak menimbulkan alert palsu.

use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct WeekPrice {
    pub week: String,
    pub week_start: NaiveDate,
    pub price: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct AnomalyParams {
    pub threshold_pct: f64,
    pub sigma: f64,
    pub min_dev_pct: f64,
}

impl Default for AnomalyParams {
    fn default() -> Self {
        AnomalyParams { threshold_pct: 5.0, sigma: 2.0, min_dev_pct: 2.0 }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PriceAnomaly {
    pub week: String,
    pub week_start: NaiveDate,
    pub price: f64,
    pub prev_price: Option<f64>,
    pub change_pct: Option<f64>,
    pub mean12: Option<f64>,
    pub std12: Option<f64>,
    pub by_threshold: bool,
    pub by_sigma: bool,
}

pub fn mean_std(xs: &[f64]) -> Option<(f64, f64)> {
    if xs.is_empty() {
        return None;
    }
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    Some((mean, var.sqrt()))
}

/// Statistik satu minggu (dipakai juga untuk tabel komoditas).
pub fn evaluate_week(series: &[WeekPrice], idx: usize, p: AnomalyParams) -> PriceAnomaly {
    let cur = &series[idx];
    let prev = if idx > 0 { series.get(idx - 1) } else { None };
    let prev_consecutive = prev.filter(|w| (cur.week_start - w.week_start).num_days() == 7);
    let change_pct = prev_consecutive.and_then(|w| if w.price > 0.0 { Some((cur.price / w.price - 1.0) * 100.0) } else { None });
    // 12 minggu kalender sebelum minggu ini
    let window: Vec<f64> = series[..idx]
        .iter()
        .filter(|w| {
            let gap = (cur.week_start - w.week_start).num_days();
            gap > 0 && gap <= 84
        })
        .map(|w| w.price)
        .collect();
    let stats = if window.len() >= 4 { mean_std(&window) } else { None };
    let by_threshold = change_pct.map(|c| c > p.threshold_pct).unwrap_or(false);
    let by_sigma = stats
        .map(|(m, s)| cur.price > m + p.sigma * s && cur.price > m * (1.0 + p.min_dev_pct / 100.0))
        .unwrap_or(false);
    PriceAnomaly {
        week: cur.week.clone(),
        week_start: cur.week_start,
        price: cur.price,
        prev_price: prev_consecutive.map(|w| w.price),
        change_pct,
        mean12: stats.map(|s| s.0),
        std12: stats.map(|s| s.1),
        by_threshold,
        by_sigma,
    }
}

/// Semua minggu yang memicu alert. `series` harus urut naik berdasarkan minggu.
pub fn detect_price_anomalies(series: &[WeekPrice], p: AnomalyParams) -> Vec<PriceAnomaly> {
    (0..series.len())
        .map(|i| evaluate_week(series, i, p))
        .filter(|a| a.by_threshold || a.by_sigma)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn series(prices: &[f64]) -> Vec<WeekPrice> {
        let start = NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid");
        prices
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let ws = start + Duration::days(7 * i as i64);
                WeekPrice { week: crate::analytics::period::iso_week_label(ws), week_start: ws, price: *p }
            })
            .collect()
    }

    #[test]
    fn lonjakan_satu_minggu_terdeteksi() {
        let mut prices = vec![9800.0, 9810.0, 9790.0, 9805.0, 9800.0, 9795.0, 9802.0, 9808.0];
        prices.push(11_368.0); // +16%
        prices.push(9800.0);
        let alerts = detect_price_anomalies(&series(&prices), AnomalyParams::default());
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].week_start, NaiveDate::from_ymd_opt(2026, 7, 27).expect("valid"));
        assert!(alerts[0].by_threshold && alerts[0].by_sigma);
        // penurunan setelah lonjakan tidak memicu alert
    }

    #[test]
    fn harga_stabil_tidak_ada_alert() {
        let prices = vec![14_500.0, 14_520.0, 14_480.0, 14_510.0, 14_490.0, 14_530.0, 14_470.0, 14_505.0, 14_560.0];
        assert!(detect_price_anomalies(&series(&prices), AnomalyParams::default()).is_empty());
    }

    #[test]
    fn kenaikan_bertahap_terdeteksi_lewat_sigma() {
        let mut prices = vec![14_500.0; 8];
        prices.extend([14_700.0, 14_900.0, 15_200.0]); // +1.4%, +1.4%, +2%
        let alerts = detect_price_anomalies(&series(&prices), AnomalyParams::default());
        assert!(!alerts.is_empty());
        assert!(alerts.iter().all(|a| !a.by_threshold && a.by_sigma));
    }

    #[test]
    fn threshold_hanya_untuk_minggu_berurutan() {
        let mut s = series(&[100.0, 100.0, 100.0, 100.0, 106.0]);
        // geser minggu terakhir 2 minggu ke depan (ada minggu kosong)
        s[4].week_start += Duration::days(14);
        let a = evaluate_week(&s, 4, AnomalyParams::default());
        assert_eq!(a.change_pct, None);
        assert!(!a.by_threshold);
    }

    #[test]
    fn mean_std_benar() {
        let (m, s) = mean_std(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).expect("ada");
        assert!((m - 5.0).abs() < 1e-12);
        assert!((s - 2.0).abs() < 1e-12);
    }
}
