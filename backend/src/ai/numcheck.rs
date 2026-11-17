//! Pengecekan bahwa semua angka di ringkasan AI berasal dari tabel hasil.
//!
//! Setiap angka di teks ditafsirkan dengan format Indonesia (1.234,5) dan Inggris (1,234.5),
//! lengkap dengan pengali (ribu/juta/miliar/triliun). Angka dianggap valid kalau cocok dengan
//! salah satu nilai di tabel (atau angka yang muncul di teks tabel/parameter/pertanyaan, mis.
//! tahun dan nomor minggu) dengan toleransi sesuai pembulatan yang dipakai.

use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

fn number_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\d[\d.,]*\d|\d)\s*(%|persen|triliun|miliar|milyar|juta|jt|ribu|rb|m\b|t\b)?").expect("regex valid"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub value: f64,
    pub tolerance: f64,
}

fn multiplier(unit: Option<&str>) -> f64 {
    match unit.map(|u| u.to_lowercase()) {
        Some(u) if u == "triliun" || u == "t" => 1e12,
        Some(u) if u == "miliar" || u == "milyar" || u == "m" => 1e9,
        Some(u) if u == "juta" || u == "jt" => 1e6,
        Some(u) if u == "ribu" || u == "rb" => 1e3,
        _ => 1.0,
    }
}

/// Tafsiran yang mungkin untuk satu token angka.
fn interpretations(tok: &str) -> Vec<(f64, u32)> {
    let mut out = Vec::new();
    // format Indonesia: titik = ribuan, koma = desimal
    let id = tok.replace('.', "").replace(',', ".");
    // format Inggris: koma = ribuan, titik = desimal
    let en = tok.replace(',', "");
    for (s, dec_sep) in [(id, ','), (en, '.')] {
        if let Ok(v) = s.parse::<f64>() {
            let decimals = tok.rsplit_once(dec_sep).map(|(_, f)| f.len() as u32).unwrap_or(0);
            // "14.840" di format Inggris berarti 14,84 (3 desimal) — tetap valid sebagai tafsiran
            out.push((v, decimals));
        }
    }
    out
}

pub fn candidates(tok: &str, unit: Option<&str>) -> Vec<Candidate> {
    let mult = multiplier(unit);
    interpretations(tok)
        .into_iter()
        .map(|(v, d)| Candidate { value: v * mult, tolerance: 0.5 * 10f64.powi(-(d as i32)) * mult * 1.05 + 1e-9 })
        .collect()
}

/// Kumpulkan semua angka dari tabel hasil (nilai numerik + angka di dalam teks).
pub fn allowed_numbers(values: &[Value], extra_text: &[String]) -> Vec<f64> {
    let mut out = Vec::new();
    fn walk(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    out.push(f.abs());
                }
            }
            Value::String(s) => out.extend(numbers_in_text(s)),
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    for v in values {
        walk(v, &mut out);
    }
    for t in extra_text {
        out.extend(numbers_in_text(t));
    }
    out
}

/// Angka "mentah" di dalam teks (tanpa tafsiran ribuan), mis. "2026-W37" -> 2026, 37.
fn numbers_in_text(s: &str) -> Vec<f64> {
    let re = Regex::new(r"\d+(?:[.,]\d+)*").expect("regex valid");
    let mut out = Vec::new();
    for m in re.find_iter(s) {
        for (v, _) in interpretations(m.as_str()) {
            out.push(v);
        }
        for part in m.as_str().split(['.', ',']) {
            if let Ok(v) = part.parse::<f64>() {
                out.push(v);
            }
        }
    }
    out
}

/// Kembalikan daftar angka di `text` yang TIDAK ditemukan di `allowed`.
pub fn unverified_numbers(text: &str, allowed: &[f64]) -> Vec<String> {
    let mut bad = Vec::new();
    for cap in number_re().captures_iter(text) {
        let Some(tok) = cap.get(1) else { continue };
        let unit = cap.get(2).map(|m| m.as_str());
        let cands = candidates(tok.as_str(), unit);
        let ok = cands.iter().any(|c| allowed.iter().any(|a| (c.value - a).abs() <= c.tolerance.max(a.abs() * 1e-9)));
        if !ok {
            bad.push(cap.get(0).map(|m| m.as_str().trim().to_string()).unwrap_or_default());
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn allowed() -> Vec<f64> {
        allowed_numbers(&[json!([["Gula Pasir", 1_234_567_890.0, 16.04, 14_840.0, "2026-W37"]])], &["8 minggu".into()])
    }

    #[test]
    fn angka_dari_tabel_lolos() {
        let a = allowed();
        assert!(unverified_numbers("Spend gula Rp 1.234.567.890 atau sekitar Rp 1,2 miliar.", &a).is_empty());
        assert!(unverified_numbers("Harga naik 16,0% menjadi Rp 14.840/kg pada minggu 2026-W37.", &a).is_empty());
        assert!(unverified_numbers("Naik 16% dalam 8 minggu.", &a).is_empty());
        assert!(unverified_numbers("Spend Rp 1,234,567,890 (16.04 percent)", &a).is_empty());
    }

    #[test]
    fn angka_karangan_ditolak() {
        let a = allowed();
        let bad = unverified_numbers("Harga naik 21,5% menjadi Rp 15.900/kg.", &a);
        assert_eq!(bad.len(), 2, "{bad:?}");
        let bad = unverified_numbers("Spend sekitar Rp 2 miliar.", &a);
        assert_eq!(bad.len(), 1);
    }
}
