//! SpendView — dashboard spend, harga komoditas & performa vendor.
//! Satu binary: API di /api/* + menyajikan hasil build frontend (React).

mod aggregates;
mod ai;
mod analytics;
mod auth;
mod config;
mod db;
mod error;
mod filters;
mod import;
mod queries;
mod routes;
mod settings;
mod state;
mod units;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("SpendView gagal start: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let _ = dotenvy::from_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.env"));
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "spendview=info,tower_http=warn".into()))
        .init();

    let cfg = config::Config::from_env();
    tracing::info!(db = %cfg.database_url, ai = %cfg.ai_provider, "memulai SpendView");
    let pool = db::connect(&cfg).await?;
    db::seed(&pool, &cfg).await?;
    if !cfg.frontend_dist.join("index.html").exists() {
        tracing::warn!("frontend belum di-build ({}). Jalankan: npm --prefix frontend ci && npm --prefix frontend run build", cfg.frontend_dist.display());
    }
    let ai = ai::AiService::new(ai::build_provider(&cfg).await, cfg.ai_max_parallel, pool.clone());
    let state = state::AppState { db: pool, cfg: Arc::new(cfg.clone()), ai: Arc::new(ai), pending: Arc::new(Mutex::new(HashMap::new())) };
    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!("SpendView berjalan di http://{}", cfg.bind_addr);
    println!("\n  SpendView siap: http://{}\n", cfg.bind_addr);
    axum::serve(listener, routes::app(state)).await?;
    Ok(())
}
