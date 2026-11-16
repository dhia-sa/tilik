//! Import CSV: parse + validasi per baris -> preview -> commit (satu transaksi) -> rollback per batch.

pub mod parse;

use crate::analytics::period::iso_week_label;
use crate::error::{AppError, AppResult};
use crate::units::UnitTable;
use chrono::NaiveDate;
use serde::Serialize;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Data master yang dibutuhkan untuk validasi.
#[derive(Debug, Default, Clone)]
pub struct Master {
    /// lower(nama) -> (id, nama asli)
    pub categories: HashMap<String, (i64, String)>,
    pub commodities: HashMap<String, (i64, String)>,
    /// lower(nama) -> item
    pub items: HashMap<String, ItemInfo>,
    /// lower(nama) -> id
    pub vendors: HashMap<String, i64>,
    pub units: UnitTable,
    /// (po_no, item_id) yang sudah ada di database
    pub existing_po_lines: HashSet<(String, i64)>,
    /// (period, vendor_id) yang sudah ada
    pub existing_ratings: HashSet<(String, i64)>,
}

#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub id: i64,
    pub name: String,
    pub category_id: i64,
    pub commodity_id: Option<i64>,
    pub base_unit: String,
}

pub async fn load_master(db: &SqlitePool) -> AppResult<Master> {
    let mut m = Master::default();
    for (id, name) in sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM categories").fetch_all(db).await? {
        m.categories.insert(name.to_lowercase(), (id, name));
    }
    for (id, name) in sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM commodities").fetch_all(db).await? {
        m.commodities.insert(name.to_lowercase(), (id, name));
    }
    let items: Vec<(i64, String, i64, Option<i64>, String)> =
        sqlx::query_as("SELECT id, name, category_id, commodity_id, base_unit FROM items").fetch_all(db).await?;
    for (id, name, category_id, commodity_id, base_unit) in items {
        m.items.insert(name.to_lowercase(), ItemInfo { id, name, category_id, commodity_id, base_unit });
    }
    for (id, name) in sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM vendors").fetch_all(db).await? {
        m.vendors.insert(name.to_lowercase(), id);
    }
    let convs: Vec<(Option<i64>, String, String, f64)> =
        sqlx::query_as("SELECT item_id, from_unit, base_unit, to_base FROM unit_conversions").fetch_all(db).await?;
    for (item_id, from, base, f) in convs {
        m.units.add(item_id, &from, &base, f);
    }
    for (po, item) in sqlx::query_as::<_, (String, i64)>("SELECT po_no, item_id FROM purchase_orders").fetch_all(db).await? {
        m.existing_po_lines.insert((po, item));
    }
    for (p, v) in sqlx::query_as::<_, (String, i64)>("SELECT period, vendor_id FROM vendor_ratings").fetch_all(db).await? {
        m.existing_ratings.insert((p, v));
    }
    Ok(m)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    Po,
    VendorRating,
}

impl ImportKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "po" => Some(ImportKind::Po),
            "vendor_rating" => Some(ImportKind::VendorRating),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            ImportKind::Po => "po",
            ImportKind::VendorRating => "vendor_rating",
        }
    }
}

/// Item yang akan dipakai baris PO: sudah ada, atau dibuat baru saat commit.
#[derive(Debug, Clone, Serialize)]
pub struct NewItem {
    pub name: String,
    pub category_id: i64,
    pub commodity_id: Option<i64>,
    pub base_unit: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PoRow {
    pub line: usize,
    pub po_no: String,
    pub po_date: NaiveDate,
    pub vendor: String,
    pub category: String,
    pub item: String,
    pub item_id: Option<i64>,
    #[serde(skip)]
    pub new_item: Option<NewItem>,
    pub commodity: Option<String>,
    pub qty: f64,
    pub unit: String,
    pub unit_price: i64,
    pub factor: f64,
    pub base_qty: f64,
    pub base_unit: String,
    pub base_unit_price: f64,
    pub delivered_date: Option<NaiveDate>,
    pub promised_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize)]
pub struct RatingRow {
    pub line: usize,
    pub period: String,
    pub vendor: String,
    pub quality_score: f64,
    pub reject_qty: f64,
    pub total_qty: f64,
    pub feedback: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RowError {
    pub line: usize,
    pub raw: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ValidRows {
    Po(Vec<PoRow>),
    Rating(Vec<RatingRow>),
}

impl ValidRows {
    pub fn len(&self) -> usize {
        match self {
            ValidRows::Po(r) => r.len(),
            ValidRows::Rating(r) => r.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingImport {
    pub kind: ImportKind,
    pub file_name: String,
    pub user_id: i64,
    pub rows: ValidRows,
    pub errors: usize,
    pub created: Instant,
}

/// Hasil validasi sebuah file.
#[derive(Debug)]
pub struct Validated {
    pub rows: ValidRows,
    pub errors: Vec<RowError>,
    pub columns: Vec<String>,
}

pub fn validate(kind: ImportKind, data: &[u8], master: &Master) -> AppResult<Validated> {
    match kind {
        ImportKind::Po => parse::validate_po(data, master),
        ImportKind::VendorRating => parse::validate_ratings(data, master),
    }
}

#[derive(Debug, Serialize)]
pub struct CommitResult {
    pub batch_id: i64,
    pub rows_ok: usize,
    pub rows_error: usize,
    pub duration_ms: u128,
    pub new_vendors: usize,
    pub new_items: usize,
}

async fn ensure_vendor(
    tx: &mut Transaction<'_, Sqlite>,
    cache: &mut HashMap<String, i64>,
    name: &str,
    batch_id: i64,
    created: &mut usize,
) -> AppResult<i64> {
    let key = name.to_lowercase();
    if let Some(id) = cache.get(&key) {
        return Ok(*id);
    }
    let id: i64 = sqlx::query_scalar("INSERT INTO vendors(name, created_batch_id) VALUES (?, ?) RETURNING id")
        .bind(name)
        .bind(batch_id)
        .fetch_one(&mut **tx)
        .await?;
    cache.insert(key, id);
    *created += 1;
    Ok(id)
}

fn map_unique(e: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db) = &e {
        if db.is_unique_violation() {
            return AppError::BadRequest(
                "Sebagian data sudah ada di database (duplikat). Ulangi preview untuk melihat baris yang bermasalah.".into(),
            );
        }
    }
    AppError::Db(e)
}

/// Simpan baris valid dalam SATU transaksi + catat batch. Agregat dihitung ulang setelahnya.
pub async fn commit(db: &SqlitePool, p: &PendingImport) -> AppResult<CommitResult> {
    let started = Instant::now();
    let master = load_master(db).await?;
    let mut tx = db.begin().await?;
    let batch_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_batches(kind, file_name, rows_ok, rows_error, user_id) VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(p.kind.as_str())
    .bind(&p.file_name)
    .bind(p.rows.len() as i64)
    .bind(p.errors as i64)
    .bind(p.user_id)
    .fetch_one(&mut *tx)
    .await?;

    let mut vendors = master.vendors.clone();
    let (mut new_vendors, mut new_items) = (0usize, 0usize);
    match &p.rows {
        ValidRows::Po(rows) => {
            let mut items: HashMap<String, i64> = master.items.iter().map(|(k, v)| (k.clone(), v.id)).collect();
            for r in rows {
                let vendor_id = ensure_vendor(&mut tx, &mut vendors, &r.vendor, batch_id, &mut new_vendors).await?;
                let item_key = r.item.to_lowercase();
                let item_id = match items.get(&item_key) {
                    Some(id) => *id,
                    None => {
                        let ni = r.new_item.as_ref().ok_or_else(|| AppError::internal("item baru tanpa data"))?;
                        let id: i64 = sqlx::query_scalar(
                            "INSERT INTO items(name, category_id, commodity_id, base_unit, created_batch_id) VALUES (?, ?, ?, ?, ?) RETURNING id",
                        )
                        .bind(&ni.name)
                        .bind(ni.category_id)
                        .bind(ni.commodity_id)
                        .bind(&ni.base_unit)
                        .bind(batch_id)
                        .fetch_one(&mut *tx)
                        .await?;
                        items.insert(item_key, id);
                        new_items += 1;
                        id
                    }
                };
                sqlx::query(
                    "INSERT INTO purchase_orders(po_no, po_date, vendor_id, item_id, qty, unit, unit_price, amount, base_qty,
                        base_unit_price, promised_date, delivered_date, iso_week, batch_id)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&r.po_no)
                .bind(r.po_date.to_string())
                .bind(vendor_id)
                .bind(item_id)
                .bind(r.qty)
                .bind(&r.unit)
                .bind(r.unit_price)
                .bind(r.qty * r.unit_price as f64)
                .bind(r.base_qty)
                .bind(r.base_unit_price)
                .bind(r.promised_date.to_string())
                .bind(r.delivered_date.map(|d| d.to_string()))
                .bind(iso_week_label(r.po_date))
                .bind(batch_id)
                .execute(&mut *tx)
                .await
                .map_err(map_unique)?;
            }
        }
        ValidRows::Rating(rows) => {
            for r in rows {
                let vendor_id = ensure_vendor(&mut tx, &mut vendors, &r.vendor, batch_id, &mut new_vendors).await?;
                sqlx::query(
                    "INSERT INTO vendor_ratings(period, vendor_id, quality_score, reject_qty, total_qty, feedback, batch_id)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&r.period)
                .bind(vendor_id)
                .bind(r.quality_score)
                .bind(r.reject_qty)
                .bind(r.total_qty)
                .bind(&r.feedback)
                .bind(batch_id)
                .execute(&mut *tx)
                .await
                .map_err(map_unique)?;
            }
        }
    }
    tx.commit().await?;
    crate::aggregates::recompute(db).await?;
    let duration_ms = started.elapsed().as_millis();
    sqlx::query("UPDATE import_batches SET duration_ms = ? WHERE id = ?")
        .bind(duration_ms as i64)
        .bind(batch_id)
        .execute(db)
        .await?;
    Ok(CommitResult { batch_id, rows_ok: p.rows.len(), rows_error: p.errors, duration_ms, new_vendors, new_items })
}

/// Rollback batch: hapus semua baris dengan batch_id tersebut (beserta vendor/item yang dibuat
/// batch ini dan tidak dipakai data lain), lalu hitung ulang agregat.
pub async fn rollback(db: &SqlitePool, batch_id: i64) -> AppResult<(u64, u64)> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT rolled_back FROM import_batches WHERE id = ?")
        .bind(batch_id)
        .fetch_optional(db)
        .await?;
    match row {
        None => return Err(AppError::NotFound("Batch import tidak ditemukan".into())),
        Some((1,)) => return Err(AppError::BadRequest("Batch ini sudah di-rollback".into())),
        _ => {}
    }
    let mut tx = db.begin().await?;
    let po = sqlx::query("DELETE FROM purchase_orders WHERE batch_id = ?").bind(batch_id).execute(&mut *tx).await?.rows_affected();
    let rt = sqlx::query("DELETE FROM vendor_ratings WHERE batch_id = ?").bind(batch_id).execute(&mut *tx).await?.rows_affected();
    sqlx::query(
        "DELETE FROM unit_conversions WHERE item_id IN (SELECT id FROM items WHERE created_batch_id = ?
           AND id NOT IN (SELECT item_id FROM purchase_orders))",
    )
    .bind(batch_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM items WHERE created_batch_id = ? AND id NOT IN (SELECT item_id FROM purchase_orders)")
        .bind(batch_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM vendors WHERE created_batch_id = ?
           AND id NOT IN (SELECT vendor_id FROM purchase_orders) AND id NOT IN (SELECT vendor_id FROM vendor_ratings)",
    )
    .bind(batch_id)
    .execute(&mut *tx)
    .await?;
    // vendor/item yang masih dipakai batch lain: lepas kaitannya supaya tidak ikut terhapus nanti
    sqlx::query("UPDATE vendors SET created_batch_id = NULL WHERE created_batch_id = ?").bind(batch_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE items SET created_batch_id = NULL WHERE created_batch_id = ?").bind(batch_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE import_batches SET rolled_back = 1, rolled_back_at = datetime('now') WHERE id = ?")
        .bind(batch_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    crate::aggregates::recompute(db).await?;
    Ok((po, rt))
}
