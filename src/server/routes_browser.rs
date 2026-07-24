use std::time::Instant;

use axum::{extract::{Query, Path, Json, Extension}, http::StatusCode, response::IntoResponse};
use serde::Deserialize;

use crate::error::CliError;
use crate::response::ApiResponse;
use crate::server::session::SessionManager;

use crate::server::routes::respond;

#[derive(Deserialize)]
pub(crate) struct NavigateReq {
    pub(crate) session: String,
    pub(crate) url: String,
}

#[derive(Deserialize)]
pub(crate) struct SnapshotReq {
    pub(crate) session: String,
}

#[derive(Deserialize)]
pub(crate) struct ClickReq {
    pub(crate) session: String,
    pub(crate) selector: String,
}

#[derive(Deserialize)]
pub(crate) struct FillReq {
    pub(crate) session: String,
    pub(crate) selector: String,
    pub(crate) value: String,
}

#[derive(Deserialize)]
pub(crate) struct EvaluateReq {
    pub(crate) session: String,
    pub(crate) script: String,
}

#[derive(Deserialize)]
pub(crate) struct ScreenshotReq {
    pub(crate) session: String,
}

#[derive(Deserialize)]
pub(crate) struct TabsReq {
    pub(crate) session: String,
    pub(crate) action: String,
    #[serde(default)]
    pub(crate) index: Option<usize>,
    #[serde(default)]
    pub(crate) url: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct KillReq {
    pub(crate) session: String,
}

pub(crate) async fn navigate(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<NavigateReq>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    respond(manager.navigate(&req.session, &req.url).await)
}

pub(crate) async fn session_info(
    Path(session_id): Path<String>,
    Extension(manager): Extension<SessionManager>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    let started_at = Instant::now();
    let sessions = manager.list().await;
    match sessions.into_iter().find(|s| s.id == session_id) {
        Some(session) => {
            let val = serde_json::to_value(&session)
                .expect("SessionInfo implements Serialize");
            (StatusCode::OK, Json(ApiResponse::success(val, started_at)))
        }
        None => {
            let err = CliError::SessionNotFound(session_id);
            (StatusCode::NOT_FOUND, Json(ApiResponse::from_cli_error(&err, started_at)))
        }
    }
}

pub(crate) async fn snapshot(
    Extension(manager): Extension<SessionManager>,
    Query(query): Query<SnapshotReq>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    respond(manager.snapshot(&query.session).await)
}

pub(crate) async fn click(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<ClickReq>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    respond(manager.click(&req.session, &req.selector).await)
}

pub(crate) async fn fill(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<FillReq>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    respond(manager.fill(&req.session, &req.selector, &req.value).await)
}

pub(crate) async fn evaluate(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<EvaluateReq>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    respond(manager.evaluate(&req.session, &req.script).await)
}

pub(crate) async fn screenshot(
    Extension(manager): Extension<SessionManager>,
    Query(query): Query<ScreenshotReq>,
) -> impl IntoResponse {
    match manager.screenshot(&query.session).await {
        Ok(data) => (StatusCode::OK, [("Content-Type", "image/png")], data).into_response(),
        Err(e) => {
            let started_at = Instant::now();
            let status = e.status_code();
            (status, Json(ApiResponse::<()>::from_cli_error(&e, started_at))).into_response()
        }
    }
}

pub(crate) async fn tabs(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<TabsReq>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    let started_at = Instant::now();
    match req.action.as_str() {
        "list" => {
            let tabs = manager.list_tabs(&req.session).await.unwrap_or_default();
            let val = serde_json::to_value(&tabs)
                .expect("Vec<TabInfo> implements Serialize");
            (StatusCode::OK, Json(ApiResponse::success(val, started_at)))
        }
        "new" => {
            match manager.new_tab(&req.session, req.url.as_deref()).await {
                Ok(()) => (StatusCode::OK, Json(ApiResponse::success(serde_json::Value::Null, started_at))),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::from_cli_error(&e, started_at))),
            }
        }
        "close" => {
            let index = req.index.unwrap_or(0);
            match manager.close_tab(&req.session, index).await {
                Ok(()) => (StatusCode::OK, Json(ApiResponse::success(serde_json::Value::Null, started_at))),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::from_cli_error(&e, started_at))),
            }
        }
        "select" => {
            let index = req.index.unwrap_or(0);
            match manager.select_tab(&req.session, index).await {
                Ok(()) => (StatusCode::OK, Json(ApiResponse::success(serde_json::Value::Null, started_at))),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::from_cli_error(&e, started_at))),
            }
        }
        _ => {
            let err = CliError::Browser("Unknown tab action".to_string());
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::from_cli_error(&err, started_at)))
        }
    }
}

pub(crate) async fn instances(
    Extension(manager): Extension<SessionManager>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    let started_at = Instant::now();
    let sessions = manager.list().await;
    let val = serde_json::to_value(&sessions)
        .expect("Vec<SessionInfo> implements Serialize");
    (StatusCode::OK, Json(ApiResponse::success(val, started_at)))
}

pub(crate) async fn kill(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<KillReq>,
) -> (StatusCode, Json<ApiResponse<()>>) {
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