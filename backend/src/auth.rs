//! Login email + password (argon2), sesi cookie HttpOnly, dan pembatasan role di backend.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{FromRequestParts, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::request::Parts;
use axum::http::HeaderMap;
use axum::response::{AppendHeaders, IntoResponse};
use axum::Json;
use serde::{Deserialize, Serialize};

pub const COOKIE_NAME: &str = "sv_session";
const SESSION_HOURS: i64 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Viewer,
    DataAdmin,
    Admin,
}

impl Role {
    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "viewer" => Some(Role::Viewer),
            "data_admin" => Some(Role::DataAdmin),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::DataAdmin => "data_admin",
            Role::Admin => "admin",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CurrentUser {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub role: Role,
}

impl CurrentUser {
    /// Data Admin: import, master data, threshold & bobot. Admin juga boleh (superuser).
    pub fn require_data_admin(&self) -> AppResult<()> {
        if matches!(self.role, Role::DataAdmin | Role::Admin) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
    /// Kelola user & role: hanya Admin.
    pub fn require_admin(&self) -> AppResult<()> {
        if self.role == Role::Admin {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

pub fn hash_password(pw: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(pw.as_bytes(), &salt).map(|h| h.to_string()).map_err(AppError::internal)
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default().verify_password(pw.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    headers.get_all(COOKIE).iter().filter_map(|v| v.to_str().ok()).flat_map(|v| v.split(';')).find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == COOKIE_NAME && !v.is_empty()).then(|| v.to_string())
    })
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = session_token(&parts.headers).ok_or(AppError::Unauthorized)?;
        let row: Option<(i64, String, String, String)> = sqlx::query_as(
            "SELECT u.id, u.email, u.name, u.role FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token = ? AND s.expires_at > datetime('now')",
        )
        .bind(&token)
        .fetch_optional(&state.db)
        .await?;
        let (id, email, name, role) = row.ok_or(AppError::Unauthorized)?;
        let role = Role::parse(&role).ok_or(AppError::Unauthorized)?;
        Ok(CurrentUser { id, email, name, role })
    }
}

#[derive(Deserialize)]
pub struct LoginReq {
    pub email: String,
    pub password: String,
}

fn cookie(state: &AppState, value: &str, max_age: i64) -> String {
    let secure = if state.cfg.cookie_secure { "; Secure" } else { "" };
    format!("{COOKIE_NAME}={value}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age}{secure}")
}

pub async fn login(State(state): State<AppState>, Json(req): Json<LoginReq>) -> AppResult<impl IntoResponse> {
    let email = req.email.trim().to_lowercase();
    let row: Option<(i64, String, String, String, String)> =
        sqlx::query_as("SELECT id, email, name, role, password_hash FROM users WHERE lower(email) = ?")
            .bind(&email)
            .fetch_optional(&state.db)
            .await?;
    let Some((id, email, name, role, hash)) = row else {
        return Err(AppError::BadRequest("Email atau password salah".into()));
    };
    if !verify_password(&req.password, &hash) {
        return Err(AppError::BadRequest("Email atau password salah".into()));
    }
    let role = Role::parse(&role).ok_or_else(|| AppError::internal("role tidak valid"))?;
    let token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    sqlx::query("DELETE FROM sessions WHERE expires_at <= datetime('now')").execute(&state.db).await?;
    sqlx::query("INSERT INTO sessions(token, user_id, expires_at) VALUES (?, ?, datetime('now', ?))")
        .bind(&token)
        .bind(id)
        .bind(format!("+{SESSION_HOURS} hours"))
        .execute(&state.db)
        .await?;
    let user = CurrentUser { id, email, name, role };
    Ok((AppendHeaders([(SET_COOKIE, cookie(&state, &token, SESSION_HOURS * 3600))]), Json(user)))
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<impl IntoResponse> {
    if let Some(token) = session_token(&headers) {
        sqlx::query("DELETE FROM sessions WHERE token = ?").bind(token).execute(&state.db).await?;
    }
    Ok((AppendHeaders([(SET_COOKIE, cookie(&state, "", 0))]), Json(serde_json::json!({ "ok": true }))))
}

pub async fn me(user: CurrentUser) -> Json<CurrentUser> {
    Json(user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_dan_verifikasi_password() {
        let h = hash_password("rahasia123").expect("hash");
        assert!(h.starts_with("$argon2"));
        assert!(verify_password("rahasia123", &h));
        assert!(!verify_password("salah", &h));
    }

    #[test]
    fn baca_cookie_sesi() {
        let mut h = HeaderMap::new();
        h.insert(COOKIE, "a=1; sv_session=abc123; b=2".parse().expect("header"));
        assert_eq!(session_token(&h), Some("abc123".into()));
    }
}
