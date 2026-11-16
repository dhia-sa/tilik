//! Pengaturan: threshold alert, bobot scorecard, master kategori/komoditas/item/satuan, user & role.

use crate::aggregates::{recompute, renormalize};
use crate::auth::{hash_password, CurrentUser, Role};
use crate::error::{AppError, AppResult};
use crate::settings::NUMERIC_KEYS;
use crate::state::AppState;
use crate::units::{normalize_unit_name, UnitTable};
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub async fn get(State(st): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let settings: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings ORDER BY key").fetch_all(&st.db).await?;
    let cats: Vec<(i64, String, i64)> =
        sqlx::query_as("SELECT c.id, c.name, (SELECT COUNT(*) FROM items i WHERE i.category_id = c.id) FROM categories c ORDER BY c.name").fetch_all(&st.db).await?;
    let coms: Vec<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT c.id, c.name, c.base_unit, (SELECT COUNT(*) FROM items i WHERE i.commodity_id = c.id) FROM commodities c ORDER BY c.name",
    )
    .fetch_all(&st.db)
    .await?;
    let items: Vec<(i64, String, i64, String, Option<i64>, Option<String>, String, i64)> = sqlx::query_as(
        "SELECT i.id, i.name, i.category_id, c.name, i.commodity_id, m.name, i.base_unit, (SELECT COUNT(*) FROM purchase_orders po WHERE po.item_id = i.id)
         FROM items i JOIN categories c ON c.id = i.category_id LEFT JOIN commodities m ON m.id = i.commodity_id ORDER BY c.name, i.name",
    )
    .fetch_all(&st.db)
    .await?;
    let units: Vec<(i64, Option<i64>, Option<String>, String, String, f64)> = sqlx::query_as(
        "SELECT u.id, u.item_id, i.name, u.from_unit, u.base_unit, u.to_base FROM unit_conversions u LEFT JOIN items i ON i.id = u.item_id
         ORDER BY u.item_id IS NOT NULL, i.name, u.from_unit",
    )
    .fetch_all(&st.db)
    .await?;
    let users: Value = if user.role == Role::Admin {
        let rows: Vec<(i64, String, String, String, String)> =
            sqlx::query_as("SELECT id, email, name, role, created_at FROM users ORDER BY id").fetch_all(&st.db).await?;
        json!(rows.into_iter().map(|(id, email, name, role, at)| json!({"id": id, "email": email, "name": name, "role": role, "created_at": at})).collect::<Vec<_>>())
    } else {
        Value::Null
    };
    Ok(Json(json!({
        "settings": settings.into_iter().collect::<HashMap<_, _>>(),
        "categories": cats.into_iter().map(|(id, name, n)| json!({"id": id, "name": name, "items": n})).collect::<Vec<_>>(),
        "commodities": coms.into_iter().map(|(id, name, unit, n)| json!({"id": id, "name": name, "base_unit": unit, "items": n})).collect::<Vec<_>>(),
        "items": items.into_iter().map(|(id, name, cid, cname, mid, mname, unit, n)| json!({
            "id": id, "name": name, "category_id": cid, "category": cname, "commodity_id": mid, "commodity": mname, "base_unit": unit, "po_lines": n
        })).collect::<Vec<_>>(),
        "units": units.into_iter().map(|(id, item_id, item, from, base, f)| json!({
            "id": id, "item_id": item_id, "item": item, "from_unit": from, "base_unit": base, "to_base": f
        })).collect::<Vec<_>>(),
        "users": users,
    })))
}

#[derive(Deserialize)]
pub struct SettingsUpdate {
    pub values: HashMap<String, f64>,
}

pub async fn update(State(st): State<AppState>, user: CurrentUser, Json(req): Json<SettingsUpdate>) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    for (k, v) in &req.values {
        let Some((_, min, max)) = NUMERIC_KEYS.iter().find(|(key, _, _)| key == k) else {
            return Err(AppError::BadRequest(format!("Pengaturan '{k}' tidak dikenal")));
        };
        if !v.is_finite() || v < min || v > max {
            return Err(AppError::BadRequest(format!("Nilai {k} harus antara {min} dan {max}")));
        }
    }
    let current = crate::settings::Settings::load(&st.db).await?;
    let w = |k: &str, d: f64| req.values.get(k).copied().unwrap_or_else(|| current.num(k, d));
    if w("weight_on_time", 40.0) + w("weight_quality", 35.0) + w("weight_price", 25.0) <= 0.0 {
        return Err(AppError::BadRequest("Total bobot scorecard harus lebih dari 0".into()));
    }
    let mut tx = st.db.begin().await?;
    for (k, v) in &req.values {
        sqlx::query("INSERT INTO settings(key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(k)
            .bind(v.to_string())
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    recompute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct NameReq {
    pub name: String,
    pub base_unit: Option<String>,
}

fn clean_name(s: &str) -> AppResult<String> {
    let n = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() || n.len() > 100 {
        return Err(AppError::BadRequest("Nama wajib diisi (maks. 100 karakter)".into()));
    }
    Ok(n)
}

fn unique_err(e: sqlx::Error, what: &str) -> AppError {
    match &e {
        sqlx::Error::Database(d) if d.is_unique_violation() => AppError::BadRequest(format!("{what} sudah ada")),
        _ => AppError::Db(e),
    }
}

const BASE_UNITS: [&str; 3] = ["kg", "liter", "pcs"];

pub async fn add_category(State(st): State<AppState>, user: CurrentUser, Json(req): Json<NameReq>) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let name = clean_name(&req.name)?;
    let id: i64 = sqlx::query_scalar("INSERT INTO categories(name) VALUES (?) RETURNING id")
        .bind(&name)
        .fetch_one(&st.db)
        .await
        .map_err(|e| unique_err(e, "Kategori"))?;
    Ok(Json(json!({ "id": id, "name": name })))
}

pub async fn delete_category(State(st): State<AppState>, user: CurrentUser, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE category_id = ?").bind(id).fetch_one(&st.db).await?;
    if n > 0 {
        return Err(AppError::BadRequest(format!("Kategori masih dipakai {n} item")));
    }
    sqlx::query("DELETE FROM categories WHERE id = ?").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn add_commodity(State(st): State<AppState>, user: CurrentUser, Json(req): Json<NameReq>) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let name = clean_name(&req.name)?;
    let unit = req.base_unit.unwrap_or_else(|| "kg".into());
    if !BASE_UNITS.contains(&unit.as_str()) {
        return Err(AppError::BadRequest("Satuan dasar harus kg, liter, atau pcs".into()));
    }
    let id: i64 = sqlx::query_scalar("INSERT INTO commodities(name, base_unit) VALUES (?, ?) RETURNING id")
        .bind(&name)
        .bind(&unit)
        .fetch_one(&st.db)
        .await
        .map_err(|e| unique_err(e, "Komoditas"))?;
    Ok(Json(json!({ "id": id, "name": name })))
}

pub async fn delete_commodity(State(st): State<AppState>, user: CurrentUser, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE commodity_id = ?").bind(id).fetch_one(&st.db).await?;
    if n > 0 {
        return Err(AppError::BadRequest(format!("Komoditas masih dipetakan ke {n} item")));
    }
    sqlx::query("DELETE FROM commodity_weekly WHERE commodity_id = ?").bind(id).execute(&st.db).await?;
    sqlx::query("DELETE FROM commodities WHERE id = ?").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ItemUpdate {
    pub category_id: i64,
    pub commodity_id: Option<i64>,
}

pub async fn update_item(State(st): State<AppState>, user: CurrentUser, Path(id): Path<i64>, Json(req): Json<ItemUpdate>) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let base: String = sqlx::query_scalar("SELECT base_unit FROM items WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Item tidak ditemukan".into()))?;
    if let Some(cid) = req.commodity_id {
        let cu: Option<String> = sqlx::query_scalar("SELECT base_unit FROM commodities WHERE id = ?").bind(cid).fetch_optional(&st.db).await?;
        match cu {
            None => return Err(AppError::BadRequest("Komoditas tidak ditemukan".into())),
            Some(u) if u != base => return Err(AppError::BadRequest(format!("Satuan dasar item ({base}) berbeda dengan komoditas ({u})"))),
            _ => {}
        }
    }
    sqlx::query("UPDATE items SET category_id = ?, commodity_id = ? WHERE id = ?")
        .bind(req.category_id)
        .bind(req.commodity_id)
        .bind(id)
        .execute(&st.db)
        .await
        .map_err(|e| AppError::BadRequest(format!("Gagal menyimpan item: {e}")))?;
    recompute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct UnitReq {
    pub item_id: Option<i64>,
    pub from_unit: String,
    pub base_unit: String,
    pub to_base: f64,
}

pub async fn add_unit(State(st): State<AppState>, user: CurrentUser, Json(req): Json<UnitReq>) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    let from = normalize_unit_name(&req.from_unit);
    if from.is_empty() {
        return Err(AppError::BadRequest("Nama satuan wajib diisi".into()));
    }
    if !BASE_UNITS.contains(&req.base_unit.as_str()) {
        return Err(AppError::BadRequest("Satuan dasar harus kg, liter, atau pcs".into()));
    }
    if !(req.to_base.is_finite() && req.to_base > 0.0) {
        return Err(AppError::BadRequest("Faktor konversi harus lebih dari 0".into()));
    }
    if let Some(item_id) = req.item_id {
        let base: Option<String> = sqlx::query_scalar("SELECT base_unit FROM items WHERE id = ?").bind(item_id).fetch_optional(&st.db).await?;
        match base {
            None => return Err(AppError::BadRequest("Item tidak ditemukan".into())),
            Some(b) if b != req.base_unit => return Err(AppError::BadRequest(format!("Satuan dasar item adalah {b}"))),
            _ => {}
        }
    }
    sqlx::query("INSERT INTO unit_conversions(item_id, from_unit, base_unit, to_base) VALUES (?, ?, ?, ?)")
        .bind(req.item_id)
        .bind(&from)
        .bind(&req.base_unit)
        .bind(req.to_base)
        .execute(&st.db)
        .await
        .map_err(|e| unique_err(e, "Konversi satuan"))?;
    renormalize(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_unit(State(st): State<AppState>, user: CurrentUser, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    user.require_data_admin()?;
    // pastikan semua PO yang ada masih bisa dikonversi tanpa konversi ini
    let convs: Vec<(i64, Option<i64>, String, String, f64)> =
        sqlx::query_as("SELECT id, item_id, from_unit, base_unit, to_base FROM unit_conversions").fetch_all(&st.db).await?;
    let mut table = UnitTable::default();
    for (cid, item, from, base, f) in convs {
        if cid != id {
            table.add(item, &from, &base, f);
        }
    }
    let used: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT DISTINCT po.item_id, i.name, i.base_unit, po.unit FROM purchase_orders po JOIN items i ON i.id = po.item_id",
    )
    .fetch_all(&st.db)
    .await?;
    if let Some((_, name, _, unit)) = used.iter().find(|(item, _, base, unit)| table.factor(Some(*item), base, unit).is_err()) {
        return Err(AppError::BadRequest(format!("Konversi masih dipakai PO (mis. item '{name}' satuan '{unit}')")));
    }
    sqlx::query("DELETE FROM unit_conversions WHERE id = ?").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct UserReq {
    pub email: Option<String>,
    pub name: Option<String>,
    pub role: Option<String>,
    pub password: Option<String>,
}

pub async fn add_user(State(st): State<AppState>, user: CurrentUser, Json(req): Json<UserReq>) -> AppResult<Json<Value>> {
    user.require_admin()?;
    let email = req.email.unwrap_or_default().trim().to_lowercase();
    if !email.contains('@') {
        return Err(AppError::BadRequest("Email tidak valid".into()));
    }
    let name = clean_name(&req.name.unwrap_or_default())?;
    let role = Role::parse(req.role.as_deref().unwrap_or("")).ok_or_else(|| AppError::BadRequest("Role harus viewer, data_admin, atau admin".into()))?;
    let pw = req.password.unwrap_or_default();
    if pw.len() < 8 {
        return Err(AppError::BadRequest("Password minimal 8 karakter".into()));
    }
    let id: i64 = sqlx::query_scalar("INSERT INTO users(email, name, password_hash, role) VALUES (?, ?, ?, ?) RETURNING id")
        .bind(&email)
        .bind(&name)
        .bind(hash_password(&pw)?)
        .bind(role.as_str())
        .fetch_one(&st.db)
        .await
        .map_err(|e| unique_err(e, "Email"))?;
    Ok(Json(json!({ "id": id })))
}

pub async fn update_user(State(st): State<AppState>, user: CurrentUser, Path(id): Path<i64>, Json(req): Json<UserReq>) -> AppResult<Json<Value>> {
    user.require_admin()?;
    if let Some(r) = &req.role {
        let role = Role::parse(r).ok_or_else(|| AppError::BadRequest("Role tidak valid".into()))?;
        if id == user.id && role != Role::Admin {
            return Err(AppError::BadRequest("Tidak bisa menurunkan role akun sendiri".into()));
        }
        sqlx::query("UPDATE users SET role = ? WHERE id = ?").bind(role.as_str()).bind(id).execute(&st.db).await?;
        sqlx::query("DELETE FROM sessions WHERE user_id = ? AND ? <> ?").bind(id).bind(id).bind(user.id).execute(&st.db).await?;
    }
    if let Some(n) = &req.name {
        sqlx::query("UPDATE users SET name = ? WHERE id = ?").bind(clean_name(n)?).bind(id).execute(&st.db).await?;
    }
    if let Some(pw) = &req.password {
        if pw.len() < 8 {
            return Err(AppError::BadRequest("Password minimal 8 karakter".into()));
        }
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?").bind(hash_password(pw)?).bind(id).execute(&st.db).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_user(State(st): State<AppState>, user: CurrentUser, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    user.require_admin()?;
    if id == user.id {
        return Err(AppError::BadRequest("Tidak bisa menghapus akun sendiri".into()));
    }
    sqlx::query("UPDATE import_batches SET user_id = NULL WHERE user_id = ?").bind(id).execute(&st.db).await?;
    sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}
