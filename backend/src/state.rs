use crate::ai::AiService;
use crate::config::Config;
use crate::import::PendingImport;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub cfg: Arc<Config>,
    pub ai: Arc<AiService>,
    /// Hasil preview import yang menunggu konfirmasi (token -> data), kedaluwarsa 30 menit.
    pub pending: Arc<Mutex<HashMap<String, PendingImport>>>,
}
