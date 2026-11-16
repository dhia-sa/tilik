//! Scorecard vendor (0-100).
//!
//! Komponen (bobot default):
//!  - Ketepatan kirim (40%): % PO terkirim dengan delivered_date <= promised_date
//!  - Kualitas (35%): rata-rata quality_score dinormalisasi ke 0-100 ((q-1)/4*100), dikurangi % reject
//!  - Harga (25%): selisih harga vendor vs rata-rata pasar internal untuk item yang sama
//!    (tertimbang spend). Skor = 80 - 4 x selisih% (dibatasi 0..100): sama dengan pasar = 80,
//!    5% lebih murah = 100, 5% lebih mahal = 60, 20% lebih mahal = 0.
//! Kalau satu komponen tidak punya data, bobotnya dibagi ulang ke komponen lain.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Weights {
    pub on_time: f64,
    pub quality: f64,
    pub price: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Weights { on_time: 40.0, quality: 35.0, price: 25.0 }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VendorInputs {
    pub delivered: i64,
    pub on_time: i64,
    pub avg_quality: Option<f64>,
    pub reject_pct: Option<f64>,
    pub price_diff_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Score {
    pub on_time_pct: Option<f64>,
    pub on_time_score: Option<f64>,
    pub quality_score: Option<f64>,
    pub price_diff_pct: Option<f64>,
    pub price_score: Option<f64>,
    pub total: Option<f64>,
    pub label: String,
}

pub fn label_for(total: f64) -> &'static str {
    if total >= 80.0 {
        "Baik"
    } else if total >= 60.0 {
        "Perlu perhatian"
    } else {
        "Bermasalah"
    }
}

pub fn quality_component(avg_quality: f64, reject_pct: f64) -> f64 {
    (((avg_quality - 1.0) / 4.0) * 100.0 - reject_pct).clamp(0.0, 100.0)
}

pub fn price_component(diff_pct: f64) -> f64 {
    (80.0 - 4.0 * diff_pct).clamp(0.0, 100.0)
}

pub fn score(inp: &VendorInputs, w: Weights) -> Score {
    let on_time_pct = if inp.delivered > 0 { Some(inp.on_time as f64 / inp.delivered as f64 * 100.0) } else { None };
    let quality = inp.avg_quality.map(|q| quality_component(q, inp.reject_pct.unwrap_or(0.0)));
    let price = inp.price_diff_pct.map(price_component);
    let parts = [(on_time_pct, w.on_time), (quality, w.quality), (price, w.price)];
    let wsum: f64 = parts.iter().filter(|(v, _)| v.is_some()).map(|(_, w)| *w).sum();
    let total = if wsum > 0.0 {
        Some(parts.iter().filter_map(|(v, w)| v.map(|v| v * w)).sum::<f64>() / wsum)
    } else {
        None
    };
    Score {
        on_time_pct,
        on_time_score: on_time_pct,
        quality_score: quality,
        price_diff_pct: inp.price_diff_pct,
        price_score: price,
        total,
        label: total.map(label_for).unwrap_or("Tidak ada data").to_string(),
    }
}

/// Satu baris pembelian: (vendor_id, item_id, spend, base_qty).
pub type PurchaseRow = (i64, i64, f64, f64);

/// Selisih harga per vendor (%) vs harga rata-rata pasar internal per item, tertimbang spend vendor.
pub fn price_diff_by_vendor(rows: &[PurchaseRow]) -> HashMap<i64, f64> {
    let mut market: HashMap<i64, (f64, f64)> = HashMap::new();
    let mut vendor_item: HashMap<(i64, i64), (f64, f64)> = HashMap::new();
    for (v, i, spend, qty) in rows {
        let m = market.entry(*i).or_default();
        m.0 += spend;
        m.1 += qty;
        let vi = vendor_item.entry((*v, *i)).or_default();
        vi.0 += spend;
        vi.1 += qty;
    }
    let mut acc: HashMap<i64, (f64, f64)> = HashMap::new();
    for ((v, i), (spend, qty)) in vendor_item {
        let Some((ms, mq)) = market.get(&i) else { continue };
        if qty <= 0.0 || *mq <= 0.0 || *ms <= 0.0 {
            continue;
        }
        let vp = spend / qty;
        let mp = ms / mq;
        let diff = (vp - mp) / mp * 100.0;
        let a = acc.entry(v).or_default();
        a.0 += diff * spend;
        a.1 += spend;
    }
    acc.into_iter().filter(|(_, (_, s))| *s > 0.0).map(|(v, (d, s))| (v, d / s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn komponen_skor() {
        assert!((quality_component(5.0, 0.0) - 100.0).abs() < 1e-9);
        assert!((quality_component(4.2, 1.0) - 79.0).abs() < 1e-9);
        assert_eq!(quality_component(1.0, 10.0), 0.0);
        assert!((price_component(0.0) - 80.0).abs() < 1e-9);
        assert!((price_component(-5.0) - 100.0).abs() < 1e-9);
        assert!((price_component(15.0) - 20.0).abs() < 1e-9);
        assert_eq!(price_component(30.0), 0.0);
    }

    #[test]
    fn skor_total_dan_label() {
        let good = VendorInputs { delivered: 100, on_time: 95, avg_quality: Some(4.4), reject_pct: Some(1.0), price_diff_pct: Some(0.0) };
        let s = score(&good, Weights::default());
        // 95*0.4 + 84*0.35 + 80*0.25 = 38 + 29.4 + 20 = 87.4
        assert!((s.total.expect("ada") - 87.4).abs() < 1e-9);
        assert_eq!(s.label, "Baik");

        let late = VendorInputs { delivered: 100, on_time: 45, avg_quality: Some(4.3), reject_pct: Some(1.0), price_diff_pct: Some(0.0) };
        let s = score(&late, Weights::default());
        assert_eq!(s.label, "Perlu perhatian");

        let bad = VendorInputs { delivered: 100, on_time: 50, avg_quality: Some(2.5), reject_pct: Some(14.0), price_diff_pct: Some(10.0) };
        assert_eq!(score(&bad, Weights::default()).label, "Bermasalah");
    }

    #[test]
    fn bobot_berubah_peringkat_berubah() {
        let telat_murah = VendorInputs { delivered: 10, on_time: 5, avg_quality: Some(4.6), reject_pct: Some(0.0), price_diff_pct: Some(-5.0) };
        let tepat_mahal = VendorInputs { delivered: 10, on_time: 10, avg_quality: Some(4.6), reject_pct: Some(0.0), price_diff_pct: Some(10.0) };
        let w1 = Weights { on_time: 40.0, quality: 35.0, price: 25.0 };
        let w2 = Weights { on_time: 10.0, quality: 30.0, price: 60.0 };
        let a1 = score(&telat_murah, w1).total.expect("ada");
        let b1 = score(&tepat_mahal, w1).total.expect("ada");
        let a2 = score(&telat_murah, w2).total.expect("ada");
        let b2 = score(&tepat_mahal, w2).total.expect("ada");
        assert!(b1 > a1);
        assert!(a2 > b2);
    }

    #[test]
    fn komponen_kosong_bobot_dibagi_ulang() {
        let only_delivery = VendorInputs { delivered: 4, on_time: 3, ..Default::default() };
        let s = score(&only_delivery, Weights::default());
        assert!((s.total.expect("ada") - 75.0).abs() < 1e-9);
        let none = score(&VendorInputs::default(), Weights::default());
        assert_eq!(none.total, None);
        assert_eq!(none.label, "Tidak ada data");
    }

    #[test]
    fn selisih_harga_vs_pasar() {
        // item 1: vendor A 100 kg @10.000, vendor B 100 kg @12.000 -> pasar 11.000
        let rows = vec![(1, 1, 1_000_000.0, 100.0), (2, 1, 1_200_000.0, 100.0)];
        let d = price_diff_by_vendor(&rows);
        assert!((d[&1] - (-100.0 / 11.0)).abs() < 1e-9);
        assert!((d[&2] - (100.0 / 11.0)).abs() < 1e-9);
    }
}
