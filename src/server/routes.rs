use std::sync::Arc;
use std::time::Instant;

use axum::{Router, routing::{get, post}, extract::Extension, http::StatusCode, Json};
use serde::Serialize;

use crate::config::Config;
use crate::error::ApiError;
use crate::response::ApiResponse;
use crate::search::Search;
use crate::server::session::SessionManager;

#[path = "routes_api.rs"]
pub mod routes_api;

#[path = "routes_browser.rs"]
pub mod routes_browser;

pub fn create_api_router(search: Arc<Search>, config: Arc<Config>) -> Router {
    Router::new()
        .route("/api/health", get(routes_api::health_check))
        .route("/api/search", get(routes_api::api_search))
        .route("/api/fetch", post(routes_api::api_fetch))
        .layer(Extension(search))
        .layer(Extension(config))
}

pub fn create_router(session_manager: SessionManager, search: Arc<Search>, config: Arc<Config>) -> Router {
    create_api_router(search, config)
        .route("/api/session/{id}", get(routes_browser::session_info))
        .route("/api/navigate", post(routes_browser::navigate))
        .route("/api/snapshot", get(routes_browser::snapshot))
        .route("/api/click", post(routes_browser::click))
        .route("/api/fill", post(routes_browser::fill))
        .route("/api/evaluate", post(routes_browser::evaluate))
        .route("/api/screenshot", get(routes_browser::screenshot))
        .route("/api/tabs", post(routes_browser::tabs))
        .route("/api/instances", get(routes_browser::instances))
        .route("/api/kill", post(routes_browser::kill))
        .layer(Extension(session_manager))
}

pub(crate) fn respond<T: Serialize>(result: crate::error::Result<T>) -> (StatusCode, Json<ApiResponse<T>>) {
    let started_at = Instant::now();
    match result {
        Ok(data) => (StatusCode::OK, Json(ApiResponse::success(data, started_at))),
        Err(e) => {
            let status = e.status_code();
            (status, Json(ApiResponse::from_cli_error(&e, started_at)))
        }
    }
}

pub(crate) fn bad_request_response<T: Serialize>(message: &str) -> (StatusCode, Json<ApiResponse<T>>) {
    let started_at = Instant::now();
    let api_error = ApiError {
        code: "bad_request".to_string(),
        message: message.to_string(),
        retryable: false,
        hint: None,
    };
    (StatusCode::BAD_REQUEST, Json(ApiResponse::error(api_error, started_at)))
}