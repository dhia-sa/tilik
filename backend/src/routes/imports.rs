//! Endpoint import: preview (validasi, belum disimpan) -> commit -> riwayat & rollback.

use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::import::{self, ImportKind, PendingImport, ValidRows};
use crate::state::AppState;
use axum::extract::{Multipart, Path, State};
use axum::Json;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const PREVIEW_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;

pub async fn preview(State(st): State<AppState>, user: CurrentUser, mut mp: Multipart) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let mut kind: Option<ImportKind> = None;
    let mut file: Option<(String, Vec<u8>)> = None;
    while let Some(field) = mp.next_field().await.map_err(|e| AppError::BadRequest(format!("Upload gagal: {e}")))? {
        match field.name() {
            Some("kind") => {
                let v = field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?;
                kind = Some(
                    ImportKind::parse(v.trim()).ok_or_else(|| AppError::BadRequest("Jenis import harus po atau vendor_rating".into()))?,
                );
            }
            Some("file") => {
                let name = field.file_name().unwrap_or("upload.csv").to_string();
                let bytes = field.bytes().await.map_err(|e| AppError::BadRequest(format!("Upload gagal: {e}")))?;
                if bytes.len() > MAX_FILE_BYTES {
                    return Err(AppError::BadRequest("File terlalu besar (maks. 20 MB)".into()));
                }
                file = Some((name, bytes.to_vec()));
            }
            _ => {}
        }
    }
    let kind = kind.ok_or_else(|| AppError::BadRequest("Pilih jenis import".into()))?;
    let (file_name, data) = file.ok_or_else(|| AppError::BadRequest("File CSV belum dipilih".into()))?;
    let started = Instant::now();
    let master = import::load_master(&st.db).await?;
    let v = import::validate(kind, &data, &master)?;
    let token = uuid::Uuid::new_v4().simple().to_string();
    let valid_count = v.rows.len();
    let sample: Value = match &v.rows {
        ValidRows::Po(r) => json!(r.iter().take(100).collect::<Vec<_>>()),
        ValidRows::Rating(r) => json!(r.iter().take(100).collect::<Vec<_>>()),
    };
    let new_vendors: Vec<String> = match &v.rows {
        ValidRows::Po(r) => r.iter().map(|x| x.vendor.clone()).collect::<Vec<_>>(),
        ValidRows::Rating(r) => r.iter().map(|x| x.vendor.clone()).collect::<Vec<_>>(),
    }
    .into_iter()
    .filter(|n| !master.vendors.contains_key(&n.to_lowercase()))
    .collect::<std::collections::BTreeSet<_>>()
    .into_iter()
    .collect();
    let errors_count = v.errors.len();
    {
        let mut pending = st.pending.lock().await;
        pending.retain(|_, p| p.created.elapsed() < PREVIEW_TTL);
        pending.insert(
            token.clone(),
            PendingImport {
                kind,
                file_name: file_name.clone(),
                user_id: user.id,
                rows: v.rows,
                errors: errors_count,
                created: Instant::now(),
            },
        );
    }
    Ok(Json(json!({
        "token": token,
        "kind": kind,
        "file_name": file_name,
        "rows_ok": valid_count,
        "rows_error": errors_count,
        "columns": v.columns,
        "valid_sample": sample,
        "errors": v.errors,
        "new_vendors": new_vendors,
        "duration_ms": started.elapsed().as_millis(),
    })))
}

pub async fn commit(State(st): State<AppState>, user: CurrentUser, Path(token): Path<String>) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let pending = st
        .pending
        .lock()
        .await
        .remove(&token)
        .ok_or_else(|| AppError::BadRequest("Preview tidak ditemukan atau sudah kedaluwarsa. Upload ulang file.".into()))?;
    if pending.rows.len() == 0 {
        return Err(AppError::BadRequest("Tidak ada baris valid untuk disimpan".into()));
    }
    let r = import::commit(&st.db, &pending).await?;
    Ok(Json(json!(r)))
}

pub async fn list(State(st): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let rows: Vec<(i64, String, String, i64, i64, Option<String>, String, i64, Option<String>, i64)> = sqlx::query_as(
        "SELECT b.id, b.kind, b.file_name, b.rows_ok, b.rows_error, u.name, b.created_at, b.rolled_back, b.rolled_back_at, b.duration_ms
         FROM import_batches b LEFT JOIN users u ON u.id = b.user_id ORDER BY b.id DESC",
    )
    .fetch_all(&st.db)
    .await?;
    let out: Vec<Value> = rows
        .into_iter()
        .map(|(id, kind, file, ok, err, user, created, rb, rb_at, ms)| {
            json!({ "id": id, "kind": kind, "file_name": file, "rows_ok": ok, "rows_error": err, "user": user,
                    "created_at": created, "rolled_back": rb == 1, "rolled_back_at": rb_at, "duration_ms": ms })
        })
        .collect();
    Ok(Json(json!({ "batches": out })))
}

pub async fn rollback(State(st): State<AppState>, user: CurrentUser, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let (po, rt) = import::rollback(&st.db, id).await?;
    Ok(Json(json!({ "ok": true, "deleted_po_lines": po, "deleted_ratings": rt })))
}
