//! Utilitas tanggal: parsing ketat, minggu ISO (Senin-Minggu), periode pembanding.

use chrono::{Datelike, Duration, NaiveDate};

/// Parse tanggal format YYYY-MM-DD secara ketat (menolak 2026-02-30, 31/11/2026, dll).
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if s.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// Label minggu ISO, mis. "2026-W37". Minggu = Senin s/d Minggu.
pub fn iso_week_label(d: NaiveDate) -> String {
    let w = d.iso_week();
    format!("{}-W{:02}", w.year(), w.week())
}

/// Tanggal Senin dari minggu ISO tanggal tersebut.
pub fn week_start(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

/// Senin dari label "YYYY-Www".
pub fn week_label_start(label: &str) -> Option<NaiveDate> {
    let (y, w) = label.split_once("-W")?;
    let y: i32 = y.parse().ok()?;
    let w: u32 = w.parse().ok()?;
    NaiveDate::from_isoywd_opt(y, w, chrono::Weekday::Mon)
}

/// Periode pembanding: rentang dengan panjang sama tepat sebelum `from`.
pub fn previous_period(from: NaiveDate, to: NaiveDate) -> (NaiveDate, NaiveDate) {
    let days = (to - from).num_days() + 1;
    let prev_to = from - Duration::days(1);
    let prev_from = prev_to - Duration::days(days - 1);
    (prev_from, prev_to)
}

/// Hari pertama bulan.
pub fn month_start(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap_or(d)
}

/// Tambah/kurang bulan pada tanggal 1.
pub fn add_months(d: NaiveDate, months: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + months;
    let y = total.div_euclid(12);
    let m = total.rem_euclid(12) as u32 + 1;
    NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(d)
}

/// Hari terakhir bulan.
pub fn month_end(d: NaiveDate) -> NaiveDate {
    add_months(month_start(d), 1) - Duration::days(1)
}

/// Parse periode "YYYY-MM" (bulan) atau "YYYY-Www" (minggu) menjadi rentang tanggal.
pub fn parse_period(s: &str) -> Option<(NaiveDate, NaiveDate)> {
    let s = s.trim();
    if s.contains("-W") {
        let start = week_label_start(s)?;
        return Some((start, start + Duration::days(6)));
    }
    let start = NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d").ok()?;
    Some((start, month_end(start)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("tanggal test valid")
    }

    #[test]
    fn parse_tanggal_ketat() {
        assert_eq!(parse_date("2026-11-10"), Some(d("2026-11-10")));
        assert_eq!(parse_date("2026-02-30"), None);
        assert_eq!(parse_date("31/11/2026"), None);
        assert_eq!(parse_date("2026-13-05"), None);
        assert_eq!(parse_date("2026-1-5"), None);
        assert_eq!(parse_date(""), None);
    }

    #[test]
    fn minggu_iso_senin_minggu() {
        // 7-13 Sep 2026 = 2026-W37
        assert_eq!(iso_week_label(d("2026-09-07")), "2026-W37");
        assert_eq!(iso_week_label(d("2026-09-13")), "2026-W37");
        assert_eq!(iso_week_label(d("2026-09-14")), "2026-W38");
        // pergantian tahun: 1 Jan 2027 (Jumat) masih 2026-W53
        assert_eq!(iso_week_label(d("2027-01-01")), "2026-W53");
        assert_eq!(week_start(d("2026-09-13")), d("2026-09-07"));
        assert_eq!(week_label_start("2026-W37"), Some(d("2026-09-07")));
    }

    #[test]
    fn periode_sebelumnya() {
        assert_eq!(previous_period(d("2026-10-01"), d("2026-10-31")), (d("2026-08-31"), d("2026-09-30")));
        assert_eq!(add_months(d("2026-01-01"), -2), d("2025-11-01"));
        assert_eq!(month_end(d("2026-02-10")), d("2026-02-28"));
        assert_eq!(parse_period("2026-10"), Some((d("2026-10-01"), d("2026-10-31"))));
        assert_eq!(parse_period("2026-W46"), Some((d("2026-11-09"), d("2026-11-15"))));
    }
}
