//! Filter global (rentang tanggal, kategori, vendor) dan tanggal acuan.

use crate::analytics::period::{add_months, month_start, parse_date, previous_period};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use chrono::{FixedOffset, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Default, Clone)]
pub struct FilterQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub category: Option<String>,
    pub vendor: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Filter {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub prev_from: NaiveDate,
    pub prev_to: NaiveDate,
    pub category: Option<i64>,
    pub vendor: Option<i64>,
}

impl Filter {
    pub fn from_s(&self) -> String {
        self.from.to_string()
    }
    pub fn to_s(&self) -> String {
        self.to.to_string()
    }
}

/// "Hari ini" untuk data: REFERENCE_DATE, atau tanggal PO terakhir, atau hari ini (WIB).
pub async fn reference_date(state: &AppState) -> AppResult<NaiveDate> {
    if let Some(d) = state.cfg.reference_date.as_deref().and_then(parse_date) {
        return Ok(d);
    }
    let max: Option<String> = sqlx::query_scalar("SELECT MAX(po_date) FROM purchase_orders").fetch_one(&state.db).await?;
    if let Some(d) = max.as_deref().and_then(parse_date) {
        return Ok(d);
    }
    let wib = FixedOffset::east_opt(7 * 3600).ok_or_else(|| AppError::internal("offset WIB"))?;
    Ok(Utc::now().with_timezone(&wib).date_naive())
}

fn opt_id(s: &Option<String>, name: &str) -> AppResult<Option<i64>> {
    match s.as_deref().map(str::trim) {
        None | Some("") | Some("all") => Ok(None),
        Some(v) => v.parse::<i64>().map(Some).map_err(|_| AppError::BadRequest(format!("{name} tidak valid"))),
    }
}

/// Default: 3 bulan terakhir (awal bulan 2 bulan lalu s/d tanggal acuan).
pub fn default_range(reference: NaiveDate) -> (NaiveDate, NaiveDate) {
    (add_months(month_start(reference), -2), reference)
}

pub async fn resolve(state: &AppState, q: &FilterQuery) -> AppResult<Filter> {
    let reference = reference_date(state).await?;
    let (dfrom, dto) = default_range(reference);
    let parse = |v: &Option<String>, def: NaiveDate, name: &str| -> AppResult<NaiveDate> {
        match v.as_deref().map(str::trim) {
            None | Some("") => Ok(def),
            Some(s) => parse_date(s).ok_or_else(|| AppError::BadRequest(format!("{name} harus berformat YYYY-MM-DD"))),
        }
    };
    let from = parse(&q.from, dfrom, "from")?;
    let to = parse(&q.to, dto, "to")?;
    if from > to {
        return Err(AppError::BadRequest("Tanggal awal harus sebelum tanggal akhir".into()));
    }
    let (prev_from, prev_to) = previous_period(from, to);
    Ok(Filter { from, to, prev_from, prev_to, category: opt_id(&q.category, "category")?, vendor: opt_id(&q.vendor, "vendor")? })
}
