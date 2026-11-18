//! Deteksi kemungkinan split PO: pembelian yang dipecah menjadi beberapa PO kecil supaya
//! tiap PO berada di bawah batas approval.
//!
//! Satu grup (vendor yang sama, tanggal PO yang sama) ditandai kalau:
//!  1. jumlah PO ≥ `min_count` (default 3),
//!  2. setiap PO di bawah batas approval (PO yang sudah di atas batas memang perlu approval),
//!  3. total semua PO di grup ≥ batas approval.
//!
//! Hasilnya "kemungkinan", bukan kesimpulan: item yang muncul di lebih dari satu PO dalam
//! grup ditampilkan sebagai petunjuk tambahan untuk ditinjau manual.

use chrono::NaiveDate;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy)]
pub struct SplitParams {
    pub approval_limit: f64,
    pub min_count: usize,
}

impl Default for SplitParams {
    fn default() -> Self {
        SplitParams { approval_limit: 100_000_000.0, min_count: 3 }
    }
}

/// Total satu PO (semua baris item dengan po_no yang sama).
#[derive(Debug, Clone)]
pub struct PoTotal {
    pub po_no: String,
    pub vendor_id: i64,
    pub vendor: String,
    pub po_date: NaiveDate,
    pub amount: f64,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SplitPo {
    pub po_no: String,
    pub amount: f64,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SplitGroup {
    pub vendor_id: i64,
    pub vendor: String,
    pub po_date: NaiveDate,
    pub po_count: usize,
    pub total: f64,
    pub largest_po: f64,
    /// Total di atas batas approval, dalam persen batas.
    pub over_limit_pct: f64,
    /// Item yang dibeli di lebih dari satu PO dalam grup ini.
    pub repeated_items: Vec<String>,
    pub pos: Vec<SplitPo>,
}

pub fn detect_split_po(pos: Vec<PoTotal>, p: SplitParams) -> Vec<SplitGroup> {
    if p.approval_limit <= 0.0 {
        return Vec::new();
    }
    let mut groups: BTreeMap<(i64, NaiveDate), Vec<PoTotal>> = BTreeMap::new();
    for po in pos {
        groups.entry((po.vendor_id, po.po_date)).or_default().push(po);
    }
    let mut out: Vec<SplitGroup> = groups
        .into_values()
        .filter_map(|mut g| {
            let total: f64 = g.iter().map(|x| x.amount).sum();
            let largest = g.iter().map(|x| x.amount).fold(0.0, f64::max);
            if g.len() < p.min_count.max(2) || largest >= p.approval_limit || total < p.approval_limit {
                return None;
            }
            let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
            for po in &g {
                for it in po.items.iter().collect::<BTreeSet<_>>() {
                    *seen.entry(it.as_str()).or_default() += 1;
                }
            }
            let repeated_items = seen.into_iter().filter(|(_, n)| *n >= 2).map(|(k, _)| k.to_string()).collect();
            g.sort_by(|a, b| b.amount.total_cmp(&a.amount).then_with(|| a.po_no.cmp(&b.po_no)));
            let first = &g[0];
            Some(SplitGroup {
                vendor_id: first.vendor_id,
                vendor: first.vendor.clone(),
                po_date: first.po_date,
                po_count: g.len(),
                total,
                largest_po: largest,
                over_limit_pct: (total / p.approval_limit - 1.0) * 100.0,
                repeated_items,
                pos: g.into_iter().map(|x| SplitPo { po_no: x.po_no, amount: x.amount, items: x.items }).collect(),
            })
        })
        .collect();
    // terbaru dulu, lalu total terbesar
    out.sort_by(|a, b| b.po_date.cmp(&a.po_date).then_with(|| b.total.total_cmp(&a.total)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, day).expect("valid")
    }

    fn po(no: &str, vendor_id: i64, day: u32, amount: f64, items: &[&str]) -> PoTotal {
        PoTotal {
            po_no: no.into(),
            vendor_id,
            vendor: format!("Vendor {vendor_id}"),
            po_date: d(day),
            amount,
            items: items.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn split_klasik_terdeteksi() {
        let pos = vec![
            po("A1", 1, 10, 30e6, &["Box"]),
            po("A2", 1, 10, 35e6, &["Box"]),
            po("A3", 1, 10, 40e6, &["Box", "Plastik"]),
            po("B1", 2, 10, 90e6, &["Box"]), // vendor lain, tidak digabung
        ];
        let g = detect_split_po(pos, SplitParams::default());
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].vendor_id, 1);
        assert_eq!(g[0].po_count, 3);
        assert!((g[0].total - 105e6).abs() < 1.0);
        assert!((g[0].over_limit_pct - 5.0).abs() < 1e-9);
        assert_eq!(g[0].repeated_items, vec!["Box".to_string()]);
        assert_eq!(g[0].pos[0].po_no, "A3", "PO terbesar di urutan pertama");
    }

    #[test]
    fn total_di_bawah_batas_bukan_split() {
        let pos = vec![po("A1", 1, 10, 30e6, &["Box"]), po("A2", 1, 10, 30e6, &["Box"]), po("A3", 1, 10, 30e6, &["Box"])];
        assert!(detect_split_po(pos, SplitParams::default()).is_empty());
    }

    #[test]
    fn satu_po_sudah_di_atas_batas_bukan_split() {
        let pos = vec![po("A1", 1, 10, 120e6, &["Box"]), po("A2", 1, 10, 10e6, &["Box"]), po("A3", 1, 10, 10e6, &["Box"])];
        assert!(detect_split_po(pos, SplitParams::default()).is_empty());
    }

    #[test]
    fn jumlah_po_kurang_dari_minimum() {
        let pos = vec![po("A1", 1, 10, 60e6, &["Box"]), po("A2", 1, 10, 60e6, &["Box"])];
        assert!(detect_split_po(pos.clone(), SplitParams::default()).is_empty());
        let g = detect_split_po(pos, SplitParams { min_count: 2, ..SplitParams::default() });
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn hari_berbeda_tidak_digabung() {
        let pos = vec![po("A1", 1, 10, 40e6, &["Box"]), po("A2", 1, 11, 40e6, &["Box"]), po("A3", 1, 12, 40e6, &["Box"])];
        assert!(detect_split_po(pos, SplitParams::default()).is_empty());
    }

    #[test]
    fn urut_terbaru_dulu() {
        let mut pos = vec![];
        for (i, day) in [3u32, 9].iter().enumerate() {
            for k in 0..3 {
                pos.push(po(&format!("P{i}{k}"), 1, *day, 40e6, &["Box"]));
            }
        }
        let g = detect_split_po(pos, SplitParams::default());
        assert_eq!(g.iter().map(|x| x.po_date).collect::<Vec<_>>(), vec![d(9), d(3)]);
    }
}
