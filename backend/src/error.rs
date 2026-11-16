//! Tipe error aplikasi. Semua handler mengembalikan `AppResult<T>` sehingga
//! tidak ada `unwrap()` di jalur request: error diubah menjadi respons JSON.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("Silakan login terlebih dahulu")]
    Unauthorized,
    #[error("Anda tidak punya akses untuk fitur ini")]
    Forbidden,
    #[error("{0}")]
    NotFound(String),
    #[error("Kesalahan database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("{0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn internal(e: impl std::fmt::Display) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Db(_) | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %self, "request gagal");
        }
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}
