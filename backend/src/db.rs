//! Koneksi database, migrasi otomatis, dan seed awal (akun demo, master data, data dummy).

use crate::auth::hash_password;
use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::import::{self, ImportKind, PendingImport};
use serde::Deserialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Instant;

pub async fn connect(cfg: &Config) -> AppResult<SqlitePool> {
    if let Some(path) = cfg.database_url.strip_prefix("sqlite://") {
        let file = path.split('?').next().unwrap_or(path);
        if let Some(parent) = std::path::Path::new(file).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(AppError::internal)?;
            }
        }
    }
    let opts = SqliteConnectOptions::from_str(&cfg.database_url)
        .map_err(AppError::internal)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);
    let pool = SqlitePoolOptions::new().max_connections(8).connect_with(opts).await?;
    sqlx::migrate!("./migrations").run(&pool).await.map_err(AppError::internal)?;
    Ok(pool)
}

#[derive(Deserialize)]
struct MasterFile {
    categories: Vec<String>,
    commodities: Vec<MasterCommodity>,
    global_units: Vec<MasterUnit>,
    items: Vec<MasterItem>,
}
#[derive(Deserialize)]
struct MasterCommodity {
    name: String,
    base_unit: String,
}
#[derive(Deserialize)]
struct MasterUnit {
    from_unit: String,
    base_unit: String,
    to_base: f64,
}
#[derive(Deserialize)]
struct MasterItem {
    name: String,
    category: String,
    commodity: Option<String>,
    base_unit: String,
    karton_to_base: Option<f64>,
}

pub const DEMO_USERS: [(&str, &str, &str, &str); 3] = [
    ("viewer@spendview.local", "Vira Viewer", "viewer", "viewer123"),
    ("dataadmin@spendview.local", "Dani Data Admin", "data_admin", "dataadmin123"),
    ("admin@spendview.local", "Adi Admin IT", "admin", "admin123"),
];

pub async fn seed(pool: &SqlitePool, cfg: &Config) -> AppResult<()> {
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(pool).await?;
    if users == 0 {
        for (email, name, role, pw) in DEMO_USERS {
            sqlx::query("INSERT INTO users(email, name, password_hash, role) VALUES (?, ?, ?, ?)")
                .bind(email)
                .bind(name)
                .bind(hash_password(pw)?)
                .bind(role)
                .execute(pool)
                .await?;
        }
        tracing::info!("akun demo dibuat");
    }

    let cats: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM categories").fetch_one(pool).await?;
    if cats == 0 {
        let path = cfg.seed_dir.join("master.json");
        let text = std::fs::read_to_string(&path).map_err(|e| AppError::internal(format!("gagal membaca {}: {e}", path.display())))?;
        let master: MasterFile = serde_json::from_str(&text).map_err(AppError::internal)?;
        let mut tx = pool.begin().await?;
        for c in &master.categories {
            sqlx::query("INSERT INTO categories(name) VALUES (?)").bind(c).execute(&mut *tx).await?;
        }
        for c in &master.commodities {
            sqlx::query("INSERT INTO commodities(name, base_unit) VALUES (?, ?)")
                .bind(&c.name)
                .bind(&c.base_unit)
                .execute(&mut *tx)
                .await?;
        }
        for u in &master.global_units {
            sqlx::query("INSERT INTO unit_conversions(item_id, from_unit, base_unit, to_base) VALUES (NULL, ?, ?, ?)")
                .bind(crate::units::normalize_unit_name(&u.from_unit))
                .bind(&u.base_unit)
                .bind(u.to_base)
                .execute(&mut *tx)
                .await?;
        }
        for it in &master.items {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO items(name, category_id, commodity_id, base_unit)
                 VALUES (?, (SELECT id FROM categories WHERE name = ?), (SELECT id FROM commodities WHERE name = ?), ?) RETURNING id",
            )
            .bind(&it.name)
            .bind(&it.category)
            .bind(&it.commodity)
            .bind(&it.base_unit)
            .fetch_one(&mut *tx)
            .await?;
            if let Some(k) = it.karton_to_base {
                sqlx::query("INSERT INTO unit_conversions(item_id, from_unit, base_unit, to_base) VALUES (?, 'karton', ?, ?)")
                    .bind(id)
                    .bind(&it.base_unit)
                    .bind(k)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        tracing::info!("master data dimuat dari {}", path.display());
    }

    let batches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM import_batches").fetch_one(pool).await?;
    if batches == 0 && cfg.seed_on_start {
        let admin_id: i64 =
            sqlx::query_scalar("SELECT id FROM users WHERE role = 'data_admin' ORDER BY id LIMIT 1").fetch_one(pool).await?;
        for (kind, file) in [(ImportKind::Po, "po_12bulan.csv"), (ImportKind::VendorRating, "penilaian_vendor_12bulan.csv")] {
            let path = cfg.seed_dir.join(file);
            let data = std::fs::read(&path).map_err(|e| AppError::internal(format!("gagal membaca {}: {e}", path.display())))?;
            let master = import::load_master(pool).await?;
            let v = import::validate(kind, &data, &master)?;
            let pending = PendingImport {
                kind,
                file_name: format!("{file} (seed otomatis)"),
                user_id: admin_id,
                rows: v.rows,
                errors: v.errors.len(),
                created: Instant::now(),
            };
            let r = import::commit(pool, &pending).await?;
            tracing::info!("seed {file}: {} baris, {} error, {} ms", r.rows_ok, r.rows_error, r.duration_ms);
        }
    }
    Ok(())
}
