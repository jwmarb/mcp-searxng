use axum::{Router, routing::{get, post}, extract::Query, Json, http::StatusCode, response::IntoResponse, Extension};
use serde::Deserialize;
use serde_json::json;
use crate::server::session::SessionManager;

pub fn create_router(session_manager: SessionManager) -> Router {
    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/navigate", post(navigate))
        .route("/api/snapshot", get(snapshot))
        .route("/api/click", post(click))
        .route("/api/fill", post(fill))
        .route("/api/evaluate", post(evaluate))
        .route("/api/screenshot", get(screenshot))
        .route("/api/tabs", post(tabs))
        .route("/api/instances", get(instances))
        .route("/api/kill", post(kill))
        .layer(Extension(session_manager))
}

#[derive(Deserialize)]
struct NavigateReq {
    session: String,
    url: String,
}

#[derive(Deserialize)]
struct SnapshotReq {
    session: String,
}

#[derive(Deserialize)]
struct ClickReq {
    session: String,
    selector: String,
}

#[derive(Deserialize)]
struct FillReq {
    session: String,
    selector: String,
    value: String,
}

#[derive(Deserialize)]
struct EvaluateReq {
    session: String,
    script: String,
}

#[derive(Deserialize)]
struct ScreenshotReq {
    session: String,
}

#[derive(Deserialize)]
struct TabsReq {
    session: String,
    action: String,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct KillReq {
    session: String,
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"ok": true})))
}

async fn navigate(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<NavigateReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match manager.navigate(&req.session, &req.url).await {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

async fn snapshot(
    Extension(manager): Extension<SessionManager>,
    Query(query): Query<SnapshotReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match manager.snapshot(&query.session).await {
        Ok(content) => (StatusCode::OK, Json(json!({"content": content}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

async fn click(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<ClickReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match manager.click(&req.session, &req.selector).await {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

async fn fill(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<FillReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match manager.fill(&req.session, &req.selector, &req.value).await {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

async fn evaluate(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<EvaluateReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match manager.evaluate(&req.session, &req.script).await {
        Ok(result) => (StatusCode::OK, Json(json!({"result": result}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

async fn screenshot(
    Extension(manager): Extension<SessionManager>,
    Query(query): Query<ScreenshotReq>,
) -> impl IntoResponse {
    match manager.screenshot(&query.session).await {
        Ok(data) => (StatusCode::OK, [("Content-Type", "image/png")], data).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn tabs(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<TabsReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match req.action.as_str() {
        "list" => {
            let tabs = manager.list_tabs(&req.session).await.unwrap_or_default();
            (StatusCode::OK, Json(json!({"tabs": tabs})))
        }
        "new" => {
            match manager.new_tab(&req.session, req.url.as_deref()).await {
                Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
            }
        }
        "close" => {
            let index = req.index.unwrap_or(0);
            match manager.close_tab(&req.session, index).await {
                Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
            }
        }
        "select" => {
            let index = req.index.unwrap_or(0);
            match manager.select_tab(&req.session, index).await {
                Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
            }
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Unknown tab action"}))),
    }
}

async fn instances(
    Extension(manager): Extension<SessionManager>,
) -> (StatusCode, Json<serde_json::Value>) {
    let sessions = manager.list().await;
    (StatusCode::OK, Json(json!({"sessions": sessions})))
}

async fn kill(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<KillReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    match manager.kill(&req.session).await {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}
