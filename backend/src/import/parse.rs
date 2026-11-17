//! Parsing & validasi CSV per baris. Setiap baris error dikembalikan beserta alasannya.

use super::{ItemInfo, Master, NewItem, PoRow, RatingRow, RowError, ValidRows, Validated};
use crate::analytics::period::parse_date;
use crate::error::{AppError, AppResult};
use crate::units::{normalize, UnitError};
use std::collections::{HashMap, HashSet};

pub const PO_COLUMNS: [&str; 11] =
    ["po_no", "po_date", "vendor", "category", "item", "commodity", "qty", "unit", "unit_price", "delivered_date", "promised_date"];
pub const RATING_COLUMNS: [&str; 6] = ["period", "vendor", "quality_score", "reject_qty", "total_qty", "feedback"];

/// Parse angka secara ketat: hanya digit dengan titik desimal opsional (mis. 1234.5).
/// Menolak format lokal "1.234,50", "Rp 315.000", dan pola ribuan ambigu seperti "2.500".
pub fn parse_number(raw: &str) -> Result<f64, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("kosong".into());
    }
    let body = s.strip_prefix('-').unwrap_or(s);
    let valid_chars = !body.is_empty()
        && body.chars().all(|c| c.is_ascii_digit() || c == '.')
        && body.matches('.').count() <= 1
        && !body.starts_with('.')
        && !body.ends_with('.');
    if !valid_chars {
        return Err(format!("format angka tidak valid '{s}' (gunakan angka tanpa pemisah ribuan, titik untuk desimal)"));
    }
    if let Some((int, frac)) = body.split_once('.') {
        if frac.len() == 3 && int.len() <= 3 && int != "0" {
            return Err(format!("format angka ambigu '{s}' (titik dianggap pemisah ribuan; tulis tanpa titik)"));
        }
    }
    s.parse::<f64>().map_err(|_| format!("format angka tidak valid '{s}'"))
}

fn read_csv(data: &[u8], expected: &[&str]) -> AppResult<(Vec<String>, Vec<(usize, Vec<String>)>)> {
    // buang BOM UTF-8 kalau ada (file dari Excel)
    let data = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    let text = std::str::from_utf8(data).map_err(|_| AppError::BadRequest("File harus berupa CSV UTF-8".into()))?;
    let delim = if text.lines().next().map(|l| l.matches(';').count() > l.matches(',').count()).unwrap_or(false) { b';' } else { b',' };
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).flexible(true).trim(csv::Trim::None).from_reader(text.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| AppError::BadRequest(format!("Header CSV tidak terbaca: {e}")))?
        .iter()
        .map(|h| h.trim().to_lowercase())
        .collect();
    let missing: Vec<&str> = expected.iter().filter(|c| !headers.iter().any(|h| h == *c)).copied().collect();
    if !missing.is_empty() {
        return Err(AppError::BadRequest(format!(
            "Kolom wajib tidak ditemukan: {}. Kolom yang diharapkan: {}",
            missing.join(", "),
            expected.join(", ")
        )));
    }
    let mut rows = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        let line = i + 2; // baris 1 = header
        let rec = rec.map_err(|e| AppError::BadRequest(format!("CSV rusak di baris {line}: {e}")))?;
        let map: HashMap<&str, String> = headers.iter().map(|h| h.as_str()).zip(rec.iter().map(|v| v.to_string())).collect();
        let ordered: Vec<String> = expected.iter().map(|c| map.get(c).cloned().unwrap_or_default()).collect();
        if ordered.iter().all(|v| v.trim().is_empty()) {
            continue; // baris kosong
        }
        rows.push((line, ordered));
    }
    if rows.is_empty() {
        return Err(AppError::BadRequest("File tidak berisi data".into()));
    }
    Ok((headers, rows))
}

pub fn validate_po(data: &[u8], m: &Master) -> AppResult<Validated> {
    let (_, raw_rows) = read_csv(data, &PO_COLUMNS)?;
    let mut ok = Vec::new();
    let mut errors = Vec::new();
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    for (line, f) in raw_rows {
        let mut reasons: Vec<String> = Vec::new();
        let get = |i: usize| f[i].trim().to_string();
        let (po_no, po_date_s, vendor, category, item, commodity, qty_s, unit, price_s, delivered_s, promised_s) =
            (get(0), get(1), get(2), get(3), get(4), get(5), get(6), get(7), get(8), get(9), get(10));

        if po_no.is_empty() {
            reasons.push("po_no kosong".into());
        }
        let po_date = if po_date_s.is_empty() {
            reasons.push("po_date kosong".into());
            None
        } else {
            let d = parse_date(&po_date_s);
            if d.is_none() {
                reasons.push(format!("po_date tidak valid '{po_date_s}' (format YYYY-MM-DD)"));
            }
            d
        };
        if vendor.is_empty() {
            reasons.push("vendor kosong".into());
        }
        let cat = if category.is_empty() {
            reasons.push("category kosong".into());
            None
        } else {
            let c = m.categories.get(&category.to_lowercase()).cloned();
            if c.is_none() {
                reasons.push(format!("kategori tidak dikenal '{category}'"));
            }
            c
        };
        let com = if commodity.is_empty() {
            None
        } else {
            match m.commodities.get(&commodity.to_lowercase()) {
                Some(c) => Some(c.clone()),
                None => {
                    reasons.push(format!("komoditas tidak dikenal '{commodity}'"));
                    None
                }
            }
        };
        let existing: Option<&ItemInfo> = if item.is_empty() {
            reasons.push("item kosong".into());
            None
        } else {
            m.items.get(&item.to_lowercase())
        };
        if let (Some(it), Some((cat_id, _))) = (existing, &cat) {
            if it.category_id != *cat_id {
                let master_cat = m.categories.values().find(|(id, _)| *id == it.category_id).map(|(_, n)| n.clone()).unwrap_or_default();
                reasons.push(format!("item '{}' terdaftar di kategori '{master_cat}', bukan '{category}'", it.name));
            }
        }
        if let (Some(it), Some((com_id, com_name))) = (existing, &com) {
            if it.commodity_id != Some(*com_id) {
                reasons.push(format!("item '{}' tidak dipetakan ke komoditas '{com_name}'", it.name));
            }
        }
        let qty = match parse_number(&qty_s) {
            Ok(q) if q > 0.0 => Some(q),
            Ok(_) => {
                reasons.push(format!("qty harus lebih dari 0 (nilai: {qty_s})"));
                None
            }
            Err(e) if e == "kosong" => {
                reasons.push("qty kosong".into());
                None
            }
            Err(e) => {
                reasons.push(format!("qty: {e}"));
                None
            }
        };
        let unit_price = match parse_number(&price_s) {
            Ok(p) if p > 0.0 => Some(p.round() as i64),
            Ok(_) => {
                reasons.push(format!("unit_price harus lebih dari 0 (nilai: {price_s})"));
                None
            }
            Err(e) if e == "kosong" => {
                reasons.push("unit_price kosong".into());
                None
            }
            Err(e) => {
                reasons.push(format!("unit_price: {e}"));
                None
            }
        };
        // satuan & satuan dasar
        let mut factor_base: Option<(f64, String)> = None;
        if unit.is_empty() {
            reasons.push("unit kosong".into());
        } else if let Some(it) = existing {
            match m.units.factor(Some(it.id), &it.base_unit, &unit) {
                Ok(f) => factor_base = Some((f, it.base_unit.clone())),
                Err(e) => reasons.push(e.to_string()),
            }
        } else if !item.is_empty() {
            // item baru: satuan dasar ditentukan dari satuan global / komoditas
            match m.units.global_base_of(&unit) {
                Some(base) => match m.units.factor(None, &base, &unit) {
                    Ok(f) => factor_base = Some((f, base)),
                    Err(e) => reasons.push(e.to_string()),
                },
                None => reasons.push(UnitError::Unknown(unit.clone()).to_string()),
            }
        }
        let promised = if promised_s.is_empty() {
            reasons.push("promised_date kosong".into());
            None
        } else {
            let d = parse_date(&promised_s);
            if d.is_none() {
                reasons.push(format!("promised_date tidak valid '{promised_s}' (format YYYY-MM-DD)"));
            }
            d
        };
        let delivered = if delivered_s.is_empty() {
            None
        } else {
            let d = parse_date(&delivered_s);
            if d.is_none() {
                reasons.push(format!("delivered_date tidak valid '{delivered_s}' (format YYYY-MM-DD)"));
            }
            d
        };
        if let (Some(pd), Some(dd)) = (po_date, delivered) {
            if dd < pd {
                reasons.push(format!("delivered_date ({dd}) lebih awal dari po_date ({pd})"));
            }
        }
        if let (Some(pd), Some(pr)) = (po_date, promised) {
            if pr < pd {
                reasons.push(format!("promised_date ({pr}) lebih awal dari po_date ({pd})"));
            }
        }
        // duplikat
        if !po_no.is_empty() && !item.is_empty() {
            let key = (po_no.clone(), item.to_lowercase());
            if let Some(prev) = seen.get(&key) {
                reasons.push(format!("duplikat PO + item di file (sama dengan baris {prev})"));
            } else {
                seen.insert(key, line);
            }
            if let Some(it) = existing {
                if m.existing_po_lines.contains(&(po_no.clone(), it.id)) {
                    reasons.push(format!("PO {po_no} untuk item '{}' sudah ada di database", it.name));
                }
            }
        }

        match (reasons.is_empty(), po_date, cat, qty, unit_price, factor_base, promised) {
            (true, Some(po_date), Some((cat_id, cat_name)), Some(qty), Some(unit_price), Some((factor, base_unit)), Some(promised)) => {
                let n = normalize(qty, unit_price as f64, factor);
                let new_item = if existing.is_none() {
                    Some(NewItem {
                        name: item.clone(),
                        category_id: cat_id,
                        commodity_id: com.as_ref().map(|c| c.0),
                        base_unit: base_unit.clone(),
                    })
                } else {
                    None
                };
                ok.push(PoRow {
                    line,
                    po_no,
                    po_date,
                    vendor,
                    category: cat_name,
                    item: existing.map(|i| i.name.clone()).unwrap_or(item),
                    item_id: existing.map(|i| i.id),
                    new_item,
                    commodity: com.map(|c| c.1).or_else(|| {
                        existing
                            .and_then(|i| i.commodity_id)
                            .and_then(|cid| m.commodities.values().find(|(id, _)| *id == cid).map(|(_, n)| n.clone()))
                    }),
                    qty,
                    unit: unit.trim().to_string(),
                    unit_price,
                    factor,
                    base_qty: n.base_qty,
                    base_unit,
                    base_unit_price: n.base_unit_price,
                    delivered_date: delivered,
                    promised_date: promised,
                });
            }
            _ => {
                if reasons.is_empty() {
                    reasons.push("baris tidak valid".into());
                }
                errors.push(RowError { line, raw: f.clone(), reasons });
            }
        }
    }
    Ok(Validated { rows: ValidRows::Po(ok), errors, columns: PO_COLUMNS.iter().map(|s| s.to_string()).collect() })
}

fn valid_period(s: &str) -> bool {
    s.len() == 7 && chrono::NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d").is_ok()
}

pub fn validate_ratings(data: &[u8], m: &Master) -> AppResult<Validated> {
    let (_, raw_rows) = read_csv(data, &RATING_COLUMNS)?;
    let mut ok = Vec::new();
    let mut errors = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for (line, f) in raw_rows {
        let mut reasons = Vec::new();
        let period = f[0].trim().to_string();
        let vendor = f[1].trim().to_string();
        if !valid_period(&period) {
            reasons.push(format!("period tidak valid '{period}' (format YYYY-MM)"));
        }
        if vendor.is_empty() {
            reasons.push("vendor kosong".into());
        }
        let q = match parse_number(&f[2]) {
            Ok(q) if (1.0..=5.0).contains(&q) => Some(q),
            Ok(q) => {
                reasons.push(format!("quality_score harus 1-5 (nilai: {q})"));
                None
            }
            Err(e) => {
                reasons.push(format!("quality_score: {e}"));
                None
            }
        };
        let rej = match parse_number(&f[3]) {
            Ok(v) if v >= 0.0 => Some(v),
            Ok(_) => {
                reasons.push("reject_qty tidak boleh negatif".into());
                None
            }
            Err(e) => {
                reasons.push(format!("reject_qty: {e}"));
                None
            }
        };
        let tot = match parse_number(&f[4]) {
            Ok(v) if v > 0.0 => Some(v),
            Ok(_) => {
                reasons.push("total_qty harus lebih dari 0".into());
                None
            }
            Err(e) => {
                reasons.push(format!("total_qty: {e}"));
                None
            }
        };
        if let (Some(r), Some(t)) = (rej, tot) {
            if r > t {
                reasons.push("reject_qty lebih besar dari total_qty".into());
            }
        }
        if !vendor.is_empty() && valid_period(&period) {
            if !seen.insert((period.clone(), vendor.to_lowercase())) {
                reasons.push("duplikat period + vendor di file".into());
            }
            if let Some(vid) = m.vendors.get(&vendor.to_lowercase()) {
                if m.existing_ratings.contains(&(period.clone(), *vid)) {
                    reasons.push(format!("penilaian {vendor} periode {period} sudah ada di database"));
                }
            }
        }
        match (reasons.is_empty(), q, rej, tot) {
            (true, Some(q), Some(r), Some(t)) => ok.push(RatingRow {
                line,
                period,
                vendor,
                quality_score: q,
                reject_qty: r,
                total_qty: t,
                feedback: f[5].trim().to_string(),
            }),
            _ => errors.push(RowError { line, raw: f.clone(), reasons }),
        }
    }
    Ok(Validated { rows: ValidRows::Rating(ok), errors, columns: RATING_COLUMNS.iter().map(|s| s.to_string()).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::ItemInfo;

    fn master() -> Master {
        let mut m = Master::default();
        m.categories.insert("bahan baku".into(), (1, "Bahan Baku".into()));
        m.categories.insert("kemasan".into(), (2, "Kemasan".into()));
        m.commodities.insert("gula pasir".into(), (1, "Gula Pasir".into()));
        m.items.insert(
            "gula pasir kristal putih".into(),
            ItemInfo { id: 10, name: "Gula Pasir Kristal Putih".into(), category_id: 1, commodity_id: Some(1), base_unit: "kg".into() },
        );
        m.units.add(None, "kg", "kg", 1.0);
        m.units.add(None, "sak 50 kg", "kg", 50.0);
        m.units.add(None, "pcs", "pcs", 1.0);
        m
    }

    const HDR: &str = "po_no,po_date,vendor,category,item,commodity,qty,unit,unit_price,delivered_date,promised_date\n";

    #[test]
    fn angka_ketat() {
        assert_eq!(parse_number("1234.5"), Ok(1234.5));
        assert_eq!(parse_number("40"), Ok(40.0));
        assert_eq!(parse_number("0.250"), Ok(0.25));
        assert!(parse_number("1.234,50").is_err());
        assert!(parse_number("742.000,00").is_err());
        assert!(parse_number("Rp 315.000").is_err());
        assert!(parse_number("2.500").is_err());
        assert!(parse_number("abc").is_err());
        assert_eq!(parse_number("-10"), Ok(-10.0));
    }

    #[test]
    fn baris_valid_dinormalisasi() {
        let csv = format!("{HDR}PO-1,2026-11-10,PT A,Bahan Baku,Gula Pasir Kristal Putih,Gula Pasir,40,Sak 50 Kg,742000,,2026-11-14\n");
        let v = validate_po(csv.as_bytes(), &master()).expect("valid");
        assert!(v.errors.is_empty(), "{:?}", v.errors);
        let ValidRows::Po(rows) = v.rows else { panic!("harus PO") };
        assert_eq!(rows[0].base_qty, 2000.0);
        assert!((rows[0].base_unit_price - 14_840.0).abs() < 1e-9);
    }

    #[test]
    fn baris_error_dengan_alasan() {
        let csv = format!(
            "{HDR}\
PO-1,2026-13-05,PT A,Bahan Baku,Gula Pasir Kristal Putih,Gula Pasir,40,sak 50 kg,742000,,2026-11-14\n\
PO-2,2026-11-10,,Bahan Baku,Gula Pasir Kristal Putih,Gula Pasir,40,sak 50 kg,742000,,2026-11-14\n\
PO-3,2026-11-10,PT A,Bahan Baku,Gula Pasir Kristal Putih,Gula Pasir,\"1.234,50\",kg,14840,,2026-11-14\n\
PO-4,2026-11-10,PT A,Bahan Baku,Gula Pasir Kristal Putih,Gula Pasir,4,drum,742000,,2026-11-14\n\
PO-5,2026-11-10,PT A,Bahan Bakar,Gula Pasir Kristal Putih,Gula Pasir,4,kg,742000,,2026-11-14\n\
PO-6,2026-11-10,PT A,Bahan Baku,Gula Pasir Kristal Putih,Gula Pasir,4,pcs,742000,,2026-11-14\n"
        );
        let v = validate_po(csv.as_bytes(), &master()).expect("valid");
        assert_eq!(v.rows.len(), 0);
        assert_eq!(v.errors.len(), 6);
        assert!(v.errors[0].reasons[0].contains("po_date tidak valid"));
        assert!(v.errors[1].reasons[0].contains("vendor kosong"));
        assert!(v.errors[2].reasons[0].contains("format angka"));
        assert!(v.errors[3].reasons[0].contains("satuan tidak dikenal"));
        assert!(v.errors[4].reasons[0].contains("kategori tidak dikenal"));
        assert!(v.errors[5].reasons[0].contains("tidak cocok"));
    }

    #[test]
    fn duplikat_di_file_dan_database() {
        let mut m = master();
        m.existing_po_lines.insert(("PO-9".into(), 10));
        let csv = format!(
            "{HDR}PO-1,2026-11-10,PT A,Bahan Baku,Gula Pasir Kristal Putih,,40,kg,14840,,2026-11-14\n\
PO-1,2026-11-10,PT A,Bahan Baku,Gula Pasir Kristal Putih,,40,kg,14840,,2026-11-14\n\
PO-9,2026-11-10,PT A,Bahan Baku,Gula Pasir Kristal Putih,,40,kg,14840,,2026-11-14\n"
        );
        let v = validate_po(csv.as_bytes(), &m).expect("valid");
        assert_eq!(v.rows.len(), 1);
        assert!(v.errors[0].reasons[0].contains("duplikat"));
        assert!(v.errors[1].reasons[0].contains("sudah ada"));
    }

    #[test]
    fn kolom_wajib_hilang() {
        let r = validate_po(b"po_no,po_date\nX,2026-01-01\n", &master());
        assert!(matches!(r, Err(AppError::BadRequest(_))));
    }

    #[test]
    fn penilaian_vendor() {
        let csv = "period,vendor,quality_score,reject_qty,total_qty,feedback\n2026-10,PT A,4.5,10,1000,ok\n2026-13,PT A,4.5,10,1000,x\n2026-10,PT B,7,10,1000,x\n2026-10,PT C,4,2000,1000,x\n";
        let v = validate_ratings(csv.as_bytes(), &master()).expect("valid");
        assert_eq!(v.rows.len(), 1);
        assert_eq!(v.errors.len(), 3);
    }
}
