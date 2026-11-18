//! Router API (/api/*) + file statis hasil build frontend.

pub mod ask;
pub mod dashboard;
pub mod export;
pub mod imports;
pub mod settings;

use crate::auth;
use crate::state::AppState;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

pub fn api() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/me", get(auth::me))
        .route("/meta", get(dashboard::meta))
        .route("/overview", get(dashboard::overview))
        .route("/spend/by-category", get(dashboard::spend_by_category))
        .route("/spend/category/{id}", get(dashboard::spend_category))
        .route("/spend/category/{id}/explain", post(dashboard::explain_category))
        .route("/commodities", get(dashboard::commodities))
        .route("/commodities/{id}", get(dashboard::commodity))
        .route("/vendors/scorecard", get(dashboard::scorecard))
        .route("/vendors/split-po", get(dashboard::split_po))
        .route("/vendors/{id}", get(dashboard::vendor_detail))
        .route("/alerts", get(dashboard::alerts))
        .route("/ask", post(ask::ask))
        .route("/ask/history", get(ask::history))
        .route("/export/{file}", get(export::export))
        .route("/imports", get(imports::list))
        .route("/imports/preview", post(imports::preview))
        .route("/imports/{token}/commit", post(imports::commit))
        .route("/imports/{id}/rollback", post(imports::rollback))
        .route("/settings", get(settings::get).put(settings::update))
        .route("/master/categories", post(settings::add_category))
        .route("/master/categories/{id}", delete(settings::delete_category))
        .route("/master/commodities", post(settings::add_commodity))
        .route("/master/commodities/{id}", delete(settings::delete_commodity))
        .route("/master/items/{id}", put(settings::update_item))
        .route("/master/units", post(settings::add_unit))
        .route("/master/units/{id}", delete(settings::delete_unit))
        .route("/users", post(settings::add_user))
        .route("/users/{id}", put(settings::update_user).delete(settings::delete_user))
        .layer(DefaultBodyLimit::max(25 * 1024 * 1024))
}

pub fn app(state: AppState) -> Router {
    let dist = state.cfg.frontend_dist.clone();
    let index = dist.join("index.html");
    let spa = ServeDir::new(&dist).fallback(ServeFile::new(index));
    Router::new()
        .nest("/api", api().fallback(|| async { crate::error::AppError::NotFound("Endpoint tidak ditemukan".into()) }))
        .fallback_service(spa)
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http())
}
