//! Konfigurasi dari environment variable (lihat `.env.example`).

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub frontend_dist: PathBuf,
    pub seed_dir: PathBuf,
    pub seed_on_start: bool,
    pub ai_provider: String,
    pub claude_bin: String,
    pub codex_bin: String,
    pub ai_timeout_secs: u64,
    pub ai_max_parallel: usize,
    pub cookie_secure: bool,
    /// Tanggal acuan "hari ini" untuk data dummy. Kosong = tanggal data terakhir.
    pub reference_date: Option<String>,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| default.to_string())
}

impl Config {
    pub fn from_env() -> Self {
        // Default path relatif terhadap folder backend/ (saat compile), sehingga
        // `cargo run` bisa dijalankan dari folder mana pun.
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let default_db = root.join("data").join("tilik.db");
        let default_db_url = format!("sqlite://{}?mode=rwc", default_db.to_string_lossy().replace('\\', "/"));
        Config {
            bind_addr: env_or("BIND_ADDR", "127.0.0.1:8080"),
            database_url: env_or("DATABASE_URL", &default_db_url),
            frontend_dist: PathBuf::from(env_or("FRONTEND_DIST", &root.join("frontend").join("dist").to_string_lossy())),
            seed_dir: PathBuf::from(env_or("SEED_DIR", &root.join("seed").to_string_lossy())),
            seed_on_start: env_or("SEED_ON_START", "true") == "true",
            ai_provider: env_or("AI_PROVIDER", "claude").to_lowercase(),
            claude_bin: env_or("CLAUDE_BIN", "claude"),
            codex_bin: env_or("CODEX_BIN", "codex"),
            ai_timeout_secs: env_or("AI_TIMEOUT_SECS", "120").parse().unwrap_or(120),
            ai_max_parallel: env_or("AI_MAX_PARALLEL", "2").parse().unwrap_or(2),
            cookie_secure: env_or("COOKIE_SECURE", "false") == "true",
            reference_date: std::env::var("REFERENCE_DATE").ok().filter(|v| !v.trim().is_empty()),
        }
    }
}
