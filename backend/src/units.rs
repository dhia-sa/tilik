//! Normalisasi satuan ke satuan dasar (kg, liter, pcs).
//!
//! Aturan: konversi khusus item (mis. "karton" Mentega = 15 kg) dicek dulu,
//! lalu konversi global (mis. "sak 50 kg" = 50 kg). Konversi global hanya
//! dipakai kalau satuan dasarnya sama dengan satuan dasar item, supaya
//! "4 kg Bearing" (item berbasis pcs) ditolak.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum UnitError {
    Unknown(String),
    Mismatch { unit: String, unit_base: String, item_base: String },
}

impl std::fmt::Display for UnitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnitError::Unknown(u) => write!(f, "satuan tidak dikenal: '{u}'"),
            UnitError::Mismatch { unit, unit_base, item_base } => {
                write!(f, "satuan '{unit}' ({unit_base}) tidak cocok dengan satuan dasar item ({item_base})")
            }
        }
    }
}

/// Seragamkan penulisan satuan: huruf kecil, spasi tunggal, alias umum.
pub fn normalize_unit_name(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    // pisahkan angka yang menempel dengan satuan: "25kg" -> "25 kg", "18l" -> "18 l"
    let mut spaced = String::with_capacity(lower.len() + 4);
    let chars: Vec<char> = lower.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        spaced.push(*c);
        if c.is_ascii_digit() {
            if let Some(next) = chars.get(i + 1) {
                if next.is_alphabetic() {
                    spaced.push(' ');
                }
            }
        }
    }
    let words: Vec<String> = spaced
        .split_whitespace()
        .map(|w| match w {
            "kilogram" | "kgs" => "kg".to_string(),
            "gr" | "g" | "gram" => "gram".to_string(),
            "ltr" | "lt" | "litre" | "liter" => "liter".to_string(),
            "pc" | "buah" | "unit" => "pcs".to_string(),
            "jerrycan" | "jeriken" | "jirigen" => "jerigen".to_string(),
            other => other.to_string(),
        })
        .collect();
    let mut joined = words.join(" ");
    // di dalam nama kemasan, liter ditulis "l" (mis. "jerigen 18 l")
    if joined.starts_with("jerigen ") && joined.ends_with(" liter") {
        joined = format!("{} l", joined.trim_end_matches(" liter"));
    }
    if joined == "l" {
        joined = "liter".to_string();
    }
    joined
}

#[derive(Debug, Default, Clone)]
pub struct UnitTable {
    /// (item_id, unit) -> (base_unit, to_base)
    item_specific: HashMap<(i64, String), (String, f64)>,
    /// unit -> (base_unit, to_base)
    global: HashMap<String, (String, f64)>,
}

impl UnitTable {
    pub fn add(&mut self, item_id: Option<i64>, from_unit: &str, base_unit: &str, to_base: f64) {
        let u = normalize_unit_name(from_unit);
        match item_id {
            Some(id) => {
                self.item_specific.insert((id, u), (base_unit.to_string(), to_base));
            }
            None => {
                self.global.insert(u, (base_unit.to_string(), to_base));
            }
        }
    }

    /// Satuan dasar dari satuan global (dipakai untuk item baru yang belum ada di master).
    pub fn global_base_of(&self, unit: &str) -> Option<String> {
        self.global.get(&normalize_unit_name(unit)).map(|(b, _)| b.clone())
    }

    /// Faktor konversi satuan -> satuan dasar item.
    pub fn factor(&self, item_id: Option<i64>, item_base: &str, unit: &str) -> Result<f64, UnitError> {
        let u = normalize_unit_name(unit);
        if let Some(id) = item_id {
            if let Some((base, f)) = self.item_specific.get(&(id, u.clone())) {
                if base == item_base {
                    return Ok(*f);
                }
            }
        }
        match self.global.get(&u) {
            Some((base, f)) if base == item_base => Ok(*f),
            Some((base, _)) => {
                Err(UnitError::Mismatch { unit: unit.trim().to_string(), unit_base: base.clone(), item_base: item_base.to_string() })
            }
            None => Err(UnitError::Unknown(unit.trim().to_string())),
        }
    }
}

/// Hasil normalisasi satu baris PO.
#[derive(Debug, Clone, PartialEq)]
pub struct Normalized {
    pub base_qty: f64,
    pub base_unit_price: f64,
}

pub fn normalize(qty: f64, unit_price: f64, factor: f64) -> Normalized {
    Normalized { base_qty: qty * factor, base_unit_price: unit_price / factor }
}

/// Rata-rata harga tertimbang qty: sum(harga * qty) / sum(qty).
/// Di database rumus yang sama dihitung sebagai SUM(amount) / SUM(base_qty) (lihat `aggregates.rs`).
#[cfg_attr(not(test), allow(dead_code))]
pub fn weighted_avg_price(rows: &[(f64, f64)]) -> Option<f64> {
    let (num, den) = rows.iter().fold((0.0, 0.0), |(n, d), (price, qty)| (n + price * qty, d + qty));
    if den > 0.0 {
        Some(num / den)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> UnitTable {
        let mut t = UnitTable::default();
        t.add(None, "kg", "kg", 1.0);
        t.add(None, "gram", "kg", 0.001);
        t.add(None, "sak 25 kg", "kg", 25.0);
        t.add(None, "sak 50 kg", "kg", 50.0);
        t.add(None, "liter", "liter", 1.0);
        t.add(None, "jerigen 18 l", "liter", 18.0);
        t.add(None, "pcs", "pcs", 1.0);
        t.add(Some(7), "karton", "kg", 15.0); // Mentega: 1 karton = 15 kg
        t.add(Some(9), "karton", "pcs", 1000.0); // Plastik: 1 karton = 1000 pcs
        t
    }

    #[test]
    fn normalisasi_penulisan_satuan() {
        assert_eq!(normalize_unit_name("  Sak 50KG "), "sak 50 kg");
        assert_eq!(normalize_unit_name("sak  25 Kg"), "sak 25 kg");
        assert_eq!(normalize_unit_name("Jerigen 18 L"), "jerigen 18 l");
        assert_eq!(normalize_unit_name("jerigen 18 liter"), "jerigen 18 l");
        assert_eq!(normalize_unit_name("Ltr"), "liter");
        assert_eq!(normalize_unit_name("L"), "liter");
        assert_eq!(normalize_unit_name("Kilogram"), "kg");
        assert_eq!(normalize_unit_name("gr"), "gram");
    }

    #[test]
    fn sak_50kg_ke_harga_per_kg() {
        // contoh dari brief: 742.000 per sak 50 kg = 14.840 per kg
        let t = table();
        let f = t.factor(Some(1), "kg", "sak 50 kg").expect("konversi ada");
        let n = normalize(40.0, 742_000.0, f);
        assert_eq!(n.base_qty, 2000.0);
        assert!((n.base_unit_price - 14_840.0).abs() < 1e-9);
    }

    #[test]
    fn jerigen_dan_gram() {
        let t = table();
        let f = t.factor(Some(3), "liter", "Jerigen 18 L").expect("ada");
        let n = normalize(10.0, 315_000.0, f);
        assert_eq!(n.base_qty, 180.0);
        assert!((n.base_unit_price - 17_500.0).abs() < 1e-9);
        let g = t.factor(Some(4), "kg", "gram").expect("ada");
        let n = normalize(500.0, 62.0, g);
        assert!((n.base_qty - 0.5).abs() < 1e-12);
        assert!((n.base_unit_price - 62_000.0).abs() < 1e-6);
    }

    #[test]
    fn karton_tergantung_item() {
        let t = table();
        assert_eq!(t.factor(Some(7), "kg", "karton"), Ok(15.0));
        assert_eq!(t.factor(Some(9), "pcs", "karton"), Ok(1000.0));
        // item tanpa definisi karton -> tidak dikenal
        assert_eq!(t.factor(Some(1), "kg", "karton"), Err(UnitError::Unknown("karton".into())));
    }

    #[test]
    fn satuan_tidak_dikenal_dan_tidak_cocok() {
        let t = table();
        assert!(matches!(t.factor(Some(1), "kg", "drum"), Err(UnitError::Unknown(_))));
        assert!(matches!(t.factor(Some(2), "pcs", "kg"), Err(UnitError::Mismatch { .. })));
    }

    #[test]
    fn rata_rata_tertimbang() {
        // 100 kg @14.000 + 300 kg @15.000 = (1.400.000 + 4.500.000) / 400 = 14.750
        let avg = weighted_avg_price(&[(14_000.0, 100.0), (15_000.0, 300.0)]).expect("ada");
        assert!((avg - 14_750.0).abs() < 1e-9);
        assert_eq!(weighted_avg_price(&[]), None);
        assert_eq!(weighted_avg_price(&[(10.0, 0.0)]), None);
    }
}
