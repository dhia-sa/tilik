//! Proyeksi harga komoditas beberapa minggu ke depan dengan regresi linear sederhana.
//!
//! Model: harga(x) = a + b·x, dihitung dengan least squares atas harga mingguan dalam
//! `window` minggu kalender terakhir yang punya data. x = jarak minggu dari minggu data
//! terakhir (0 = minggu terakhir, −1 = minggu sebelumnya, …), jadi minggu tanpa PO tetap
//! dihitung jaraknya dengan benar.
//!
//! Pita ketidakpastian = prediction interval 95%:
//!   ŷ ± t(0,975; n−2) · s · √(1 + 1/n + (x₀ − x̄)² / Sxx),   s = √(SSE / (n − 2))
//! Pita melebar untuk minggu yang lebih jauh, dan lebih lebar kalau harga historisnya
//! berfluktuasi (mis. ada lonjakan satu minggu).

use crate::analytics::anomaly::WeekPrice;
use crate::analytics::period::iso_week_label;
use chrono::{Duration, NaiveDate};
use serde::Serialize;

/// Minimal jumlah minggu berdata supaya proyeksi ditampilkan.
pub const MIN_POINTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearFit {
    pub intercept: f64,
    pub slope: f64,
    pub resid_std: f64,
    pub n: usize,
    pub x_mean: f64,
    pub sxx: f64,
}

impl LinearFit {
    /// (perkiraan, batas bawah, batas atas) untuk titik x0, interval 95%.
    pub fn predict(&self, x0: f64) -> (f64, f64, f64) {
        let y = self.intercept + self.slope * x0;
        let n = self.n as f64;
        let half = t95(self.n - 2) * self.resid_std * (1.0 + 1.0 / n + (x0 - self.x_mean).powi(2) / self.sxx).sqrt();
        (y, y - half, y + half)
    }
}

/// Least squares y = a + b·x. None kalau titik < 3 atau semua x sama.
pub fn fit_linear(points: &[(f64, f64)]) -> Option<LinearFit> {
    let n = points.len();
    if n < 3 {
        return None;
    }
    let nf = n as f64;
    let x_mean = points.iter().map(|p| p.0).sum::<f64>() / nf;
    let y_mean = points.iter().map(|p| p.1).sum::<f64>() / nf;
    let sxx: f64 = points.iter().map(|p| (p.0 - x_mean).powi(2)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let sxy: f64 = points.iter().map(|p| (p.0 - x_mean) * (p.1 - y_mean)).sum();
    let slope = sxy / sxx;
    let intercept = y_mean - slope * x_mean;
    let sse: f64 = points.iter().map(|p| (p.1 - intercept - slope * p.0).powi(2)).sum();
    Some(LinearFit { intercept, slope, resid_std: (sse / (nf - 2.0)).sqrt(), n, x_mean, sxx })
}

/// Nilai t Student dua sisi 95% (t 0,975) untuk derajat bebas `df`.
pub fn t95(df: usize) -> f64 {
    const T: [f64; 30] = [
        12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160, 2.145, 2.131, 2.120, 2.110, 2.101,
        2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056, 2.052, 2.048, 2.045, 2.042,
    ];
    match df {
        0 => f64::INFINITY,
        1..=30 => T[df - 1],
        _ => 1.96,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ForecastPoint {
    pub week: String,
    pub week_start: String,
    pub price: f64,
    pub lower: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Forecast {
    pub method: String,
    pub window_weeks: i64,
    pub points_used: usize,
    pub last_week: String,
    pub last_price: f64,
    pub slope_per_week: f64,
    pub slope_pct_per_week: f64,
    pub resid_std: f64,
    pub points: Vec<ForecastPoint>,
}

/// Proyeksi `horizon` minggu setelah minggu data terakhir. `series` urut naik.
pub fn forecast_weeks(series: &[WeekPrice], window: i64, horizon: i64) -> Option<Forecast> {
    let last = series.last()?;
    let pts: Vec<(f64, f64)> = series
        .iter()
        .filter(|w| (last.week_start - w.week_start).num_days() < 7 * window)
        .map(|w| ((w.week_start - last.week_start).num_days() as f64 / 7.0, w.price))
        .collect();
    if pts.len() < MIN_POINTS {
        return None;
    }
    let fit = fit_linear(&pts)?;
    let points = (1..=horizon)
        .map(|h| {
            let ws: NaiveDate = last.week_start + Duration::days(7 * h);
            let (y, lo, hi) = fit.predict(h as f64);
            ForecastPoint { week: iso_week_label(ws), week_start: ws.to_string(), price: y, lower: lo.max(0.0), upper: hi }
        })
        .collect();
    let level = fit.intercept; // nilai garis tren di minggu terakhir (x = 0)
    Some(Forecast {
        method: format!("Regresi linear {} minggu terakhir, pita = interval prediksi 95%", window),
        window_weeks: window,
        points_used: pts.len(),
        last_week: last.week.clone(),
        last_price: last.price,
        slope_per_week: fit.slope,
        slope_pct_per_week: if level > 0.0 { fit.slope / level * 100.0 } else { 0.0 },
        resid_std: fit.resid_std,
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(prices: &[f64]) -> Vec<WeekPrice> {
        let start = NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid");
        prices
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let ws = start + Duration::days(7 * i as i64);
                WeekPrice { week: iso_week_label(ws), week_start: ws, price: *p }
            })
            .collect()
    }

    #[test]
    fn garis_lurus_diproyeksikan_tepat() {
        // naik 100 per minggu tanpa noise
        let s = series(&(0..12).map(|i| 10_000.0 + 100.0 * i as f64).collect::<Vec<_>>());
        let f = forecast_weeks(&s, 12, 4).expect("ada proyeksi");
        assert_eq!(f.points.len(), 4);
        assert!((f.slope_per_week - 100.0).abs() < 1e-9);
        for (h, p) in f.points.iter().enumerate() {
            let expect = 11_100.0 + 100.0 * (h as f64 + 1.0);
            assert!((p.price - expect).abs() < 1e-6, "minggu +{}: {}", h + 1, p.price);
            assert!(p.upper - p.lower < 1e-6, "tanpa noise, pita nol");
        }
        assert_eq!(f.points[0].week_start, "2026-08-24");
        assert_eq!(f.points[0].week, "2026-W35");
    }

    #[test]
    fn pita_melebar_untuk_minggu_lebih_jauh() {
        let s = series(&[100.0, 103.0, 99.0, 104.0, 101.0, 106.0, 102.0, 107.0, 104.0, 108.0]);
        let f = forecast_weeks(&s, 12, 4).expect("ada proyeksi");
        let widths: Vec<f64> = f.points.iter().map(|p| p.upper - p.lower).collect();
        assert!(widths.windows(2).all(|w| w[1] > w[0]), "{widths:?}");
        assert!(f.points.iter().all(|p| p.lower < p.price && p.price < p.upper));
        assert!(f.slope_per_week > 0.0);
    }

    #[test]
    fn hanya_minggu_dalam_window_yang_dipakai() {
        // 8 minggu lama di harga 50, lalu 6 minggu datar di 100: window 6 -> proyeksi datar 100
        let mut p = vec![50.0; 8];
        p.extend([100.0; 6]);
        let f = forecast_weeks(&series(&p), 6, 4).expect("ada proyeksi");
        assert_eq!(f.points_used, 6);
        assert!(f.points.iter().all(|x| (x.price - 100.0).abs() < 1e-9));
    }

    #[test]
    fn minggu_kosong_tetap_dihitung_jaraknya() {
        // harga naik 10/minggu, tapi minggu ke-3 dan ke-4 tidak ada PO
        let mut s = series(&[100.0, 110.0, 120.0, 130.0, 140.0, 150.0]);
        s.remove(3);
        s.remove(2);
        let f = forecast_weeks(&s, 12, 1).expect("ada proyeksi");
        assert!((f.points[0].price - 160.0).abs() < 1e-9);
    }

    #[test]
    fn data_terlalu_sedikit_tidak_diproyeksikan() {
        assert!(forecast_weeks(&series(&[100.0, 101.0, 102.0]), 12, 4).is_none());
        assert!(forecast_weeks(&[], 12, 4).is_none());
    }

    #[test]
    fn nilai_t_sesuai_tabel() {
        assert_eq!(t95(10), 2.228);
        assert_eq!(t95(1), 12.706);
        assert_eq!(t95(100), 1.96);
    }
}
