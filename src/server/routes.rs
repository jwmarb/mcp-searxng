use axum::{Router, routing::{get, post}, extract::Query, Json, http::StatusCode, response::IntoResponse, Extension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::error::Result;
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

fn respond<T: Serialize>(result: Result<T>) -> (StatusCode, Json<serde_json::Value>) {
    match result {
        Ok(data) => (StatusCode::OK, Json(json!({"ok": true, "data": data}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

async fn navigate(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<NavigateReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    respond(manager.navigate(&req.session, &req.url).await)
}

async fn snapshot(
    Extension(manager): Extension<SessionManager>,
    Query(query): Query<SnapshotReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    respond(manager.snapshot(&query.session).await)
}

async fn click(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<ClickReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    respond(manager.click(&req.session, &req.selector).await)
}

async fn fill(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<FillReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    respond(manager.fill(&req.session, &req.selector, &req.value).await)
}

async fn evaluate(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<EvaluateReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    respond(manager.evaluate(&req.session, &req.script).await)
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
    respond(manager.kill(&req.session).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_navigate_req_deserialize() {
        let data = json!({"session": "sess-1", "url": "https://example.com"});
        let req: NavigateReq = serde_json::from_value(data).unwrap();
        assert_eq!(req.session, "sess-1");
        assert_eq!(req.url, "https://example.com");
    }

    #[test]
    fn test_navigate_req_missing_session() {
        let data = json!({"url": "https://example.com"});
        let result: std::result::Result<NavigateReq, _> = serde_json::from_value(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_click_req_deserialize() {
        let data = json!({"session": "sess-1", "selector": "#submit"});
        let req: ClickReq = serde_json::from_value(data).unwrap();
        assert_eq!(req.session, "sess-1");
        assert_eq!(req.selector, "#submit");
    }

    #[test]
    fn test_fill_req_deserialize() {
        let data = json!({"session": "sess-1", "selector": "#input", "value": "test"});
        let req: FillReq = serde_json::from_value(data).unwrap();
        assert_eq!(req.session, "sess-1");
        assert_eq!(req.selector, "#input");
        assert_eq!(req.value, "test");
    }

    #[test]
    fn test_evaluate_req_deserialize() {
        let data = json!({"session": "sess-1", "script": "1+1"});
        let req: EvaluateReq = serde_json::from_value(data).unwrap();
        assert_eq!(req.session, "sess-1");
        assert_eq!(req.script, "1+1");
    }

    #[test]
    fn test_screenshot_req_deserialize() {
        let data = json!({"session": "sess-1"});
        let req: ScreenshotReq = serde_json::from_value(data).unwrap();
        assert_eq!(req.session, "sess-1");
    }

    #[test]
    fn test_tabs_req_with_defaults() {
        let data = json!({"session": "sess-1", "action": "list"});
        let req: TabsReq = serde_json::from_value(data).unwrap();
        assert_eq!(req.session, "sess-1");
        assert_eq!(req.action, "list");
        assert_eq!(req.index, None);
        assert_eq!(req.url, None);
    }

    #[test]
    fn test_tabs_req_with_values() {
        let data = json!({"session": "sess-1", "action": "select", "index": 2, "url": "https://new.com"});
        let req: TabsReq = serde_json::from_value(data).unwrap();
        assert_eq!(req.index, Some(2));
        assert_eq!(req.url, Some("https://new.com".to_string()));
    }

    #[test]
    fn test_kill_req_deserialize() {
        let data = json!({"session": "sess-1"});
        let req: KillReq = serde_json::from_value(data).unwrap();
        assert_eq!(req.session, "sess-1");
    }

    #[test]
    fn test_snapshot_req_deserialize() {
        let data = json!({"session": "sess-1"});
        let req: SnapshotReq = serde_json::from_value(data).unwrap();
        assert_eq!(req.session, "sess-1");
    }
}
