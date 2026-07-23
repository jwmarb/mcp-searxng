use std::sync::Arc;
use std::time::Instant;

use axum::{Router, routing::{get, post}, extract::{Query, Path}, Json, http::StatusCode, response::IntoResponse, Extension};
use serde::{Deserialize, Serialize};
use crate::config::Config;
use crate::error::{ApiError, CliError, Result};
use crate::response::ApiResponse;
use crate::server::session::SessionManager;
use crate::search::{Search, SearchParams, OutputFormat as SearchOutputFormat, SearchResponse};
use crate::fetch::{Fetcher, FetchParams, FetchResponse, RenderMode};
use crate::retry::RetryClient;

pub fn create_api_router(search: Arc<Search>, config: Arc<Config>) -> Router {
    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/search", get(api_search))
        .route("/api/fetch", post(api_fetch))
        .layer(Extension(search))
        .layer(Extension(config))
}

pub fn create_router(session_manager: SessionManager, search: Arc<Search>, config: Arc<Config>) -> Router {
    create_api_router(search, config)
        .route("/api/session/{id}", get(session_info))
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
struct SearchQueryParams {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    categories: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    time_range: Option<String>,
    #[serde(default)]
    safesearch: Option<u8>,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    max_results: Option<usize>,
}

#[derive(Deserialize)]
struct FetchRequestBody {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    max_chars: Option<usize>,
    #[serde(default)]
    timeout: Option<u64>,
    #[serde(default)]
    render: Option<bool>,
}

fn bad_request_response<T: Serialize>(message: &str) -> (StatusCode, Json<ApiResponse<T>>) {
    let started_at = Instant::now();
    let api_error = ApiError {
        code: "bad_request".to_string(),
        message: message.to_string(),
        retryable: false,
        hint: None,
    };
    (StatusCode::BAD_REQUEST, Json(ApiResponse::error(api_error, started_at)))
}

async fn api_search(
    Extension(search): Extension<Arc<Search>>,
    Query(params): Query<SearchQueryParams>,
) -> (StatusCode, Json<ApiResponse<SearchResponse>>) {
    let query = match &params.query {
        Some(q) if !q.trim().is_empty() => q.clone(),
        _ => return bad_request_response("Missing required parameter: query"),
    };

    let search_params = SearchParams {
        query,
        categories: params.categories,
        language: params.language,
        time_range: params.time_range,
        safesearch: params.safesearch,
        page: params.page,
        max_results: params.max_results,
        format: SearchOutputFormat::Json,
    };

    respond(search.search(&search_params).await)
}

async fn api_fetch(
    Extension(config): Extension<Arc<Config>>,
    Json(body): Json<FetchRequestBody>,
) -> (StatusCode, Json<ApiResponse<FetchResponse>>) {
    let url = match &body.url {
        Some(u) if !u.trim().is_empty() => u.clone(),
        _ => return bad_request_response("Missing required parameter: url"),
    };

    if url::Url::parse(&url).is_err() {
        return bad_request_response(&format!("Invalid URL: {}", url));
    }

    let retry_client = RetryClient::new(&config.retry);
    let fetcher = Fetcher::new(retry_client).with_config((*config).clone());
    let params = FetchParams {
        url,
        max_chars: body.max_chars,
        timeout: body.timeout,
        render_mode: if body.render.unwrap_or(false) {
            RenderMode::Render
        } else {
            RenderMode::Lightweight
        },
    };

    respond(fetcher.fetch(&params).await)
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

async fn health_check(
    search: Option<Extension<Arc<Search>>>,
    session_manager: Option<Extension<SessionManager>>,
) -> impl IntoResponse {
    let started_at = Instant::now();

    let searxng_reachable = if let Some(Extension(search)) = &search {
        search.ping().await
    } else {
        false
    };

    let pool_status = if let Some(Extension(manager)) = &session_manager {
        Some(manager.pool_status().await)
    } else {
        None
    };

    let health = HealthResponse {
        healthy: searxng_reachable,
        searxng_reachable,
        pool_status,
    };
    (StatusCode::OK, Json(ApiResponse::success(health, started_at)))
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    healthy: bool,
    searxng_reachable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pool_status: Option<crate::browser::pool::PoolStatus>,
}

fn respond<T: Serialize>(result: Result<T>) -> (StatusCode, Json<ApiResponse<T>>) {
    let started_at = Instant::now();
    match result {
        Ok(data) => (StatusCode::OK, Json(ApiResponse::success(data, started_at))),
        Err(e) => {
            let status = match &e {
                CliError::SessionNotFound(_) => StatusCode::NOT_FOUND,
                CliError::SessionRequired => StatusCode::BAD_REQUEST,
                CliError::ServerNotRunning => StatusCode::SERVICE_UNAVAILABLE,
                CliError::Http(_) => StatusCode::BAD_GATEWAY,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(ApiResponse::from_cli_error(&e, started_at)))
        }
    }
}

async fn navigate(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<NavigateReq>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    respond(manager.navigate(&req.session, &req.url).await)
}

async fn session_info(
    Path(session_id): Path<String>,
    Extension(manager): Extension<SessionManager>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    let started_at = Instant::now();
    let sessions = manager.list().await;
    match sessions.into_iter().find(|s| s.id == session_id) {
        Some(session) => {
            let val = serde_json::to_value(session).unwrap();
            (StatusCode::OK, Json(ApiResponse::success(val, started_at)))
        }
        None => {
            let err = CliError::SessionNotFound(session_id);
            (StatusCode::NOT_FOUND, Json(ApiResponse::from_cli_error(&err, started_at)))
        }
    }
}

async fn snapshot(
    Extension(manager): Extension<SessionManager>,
    Query(query): Query<SnapshotReq>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    respond(manager.snapshot(&query.session).await)
}

async fn click(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<ClickReq>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    respond(manager.click(&req.session, &req.selector).await)
}

async fn fill(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<FillReq>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    respond(manager.fill(&req.session, &req.selector, &req.value).await)
}

async fn evaluate(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<EvaluateReq>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    respond(manager.evaluate(&req.session, &req.script).await)
}

async fn screenshot(
    Extension(manager): Extension<SessionManager>,
    Query(query): Query<ScreenshotReq>,
) -> impl IntoResponse {
    match manager.screenshot(&query.session).await {
        Ok(data) => (StatusCode::OK, [("Content-Type", "image/png")], data).into_response(),
        Err(e) => {
            let started_at = Instant::now();
            let status = match &e {
                CliError::SessionNotFound(_) => StatusCode::NOT_FOUND,
                CliError::SessionRequired => StatusCode::BAD_REQUEST,
                CliError::ServerNotRunning => StatusCode::SERVICE_UNAVAILABLE,
                CliError::Http(_) => StatusCode::BAD_GATEWAY,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(ApiResponse::<()>::from_cli_error(&e, started_at))).into_response()
        }
    }
}

async fn tabs(
    Extension(manager): Extension<SessionManager>,
    Json(req): Json<TabsReq>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    let started_at = Instant::now();
    match req.action.as_str() {
        "list" => {
            let tabs = manager.list_tabs(&req.session).await.unwrap_or_default();
            let val = serde_json::to_value(tabs).unwrap();
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

async fn instances(
    Extension(manager): Extension<SessionManager>,
) -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    let started_at = Instant::now();
    let sessions = manager.list().await;
    let val = serde_json::to_value(sessions).unwrap();
    (StatusCode::OK, Json(ApiResponse::success(val, started_at)))
}

async fn kill(
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

    #[test]
    fn test_respond_success() {
        let result: Result<String> = Ok("test".to_string());
        let (status, body) = respond(result);
        assert_eq!(status, StatusCode::OK);
        let val: serde_json::Value = serde_json::to_value(body.0).unwrap();
        assert_eq!(val["success"], true);
        assert_eq!(val["data"], "test");
        assert!(val["error"].is_null());
        assert!(val["metadata"]["timestamp"].is_string());
    }

    #[test]
    fn test_respond_session_not_found() {
        let result: Result<String> = Err(CliError::SessionNotFound("x".to_string()));
        let (status, body) = respond(result);
        assert_eq!(status, StatusCode::NOT_FOUND);
        let val: serde_json::Value = serde_json::to_value(body.0).unwrap();
        assert_eq!(val["success"], false);
        assert!(val["data"].is_null());
        assert_eq!(val["error"]["code"], "session_not_found");
        assert!(val["error"]["message"].is_string());
    }

    #[test]
    fn test_respond_session_required() {
        let result: Result<String> = Err(CliError::SessionRequired);
        let (status, body) = respond(result);
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let val: serde_json::Value = serde_json::to_value(body.0).unwrap();
        assert_eq!(val["success"], false);
        assert!(val["data"].is_null());
        assert_eq!(val["error"]["code"], "session_required");
    }

    #[test]
    fn test_respond_server_not_running() {
        let result: Result<String> = Err(CliError::ServerNotRunning);
        let (status, body) = respond(result);
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let val: serde_json::Value = serde_json::to_value(body.0).unwrap();
        assert_eq!(val["success"], false);
        assert!(val["data"].is_null());
        assert_eq!(val["error"]["code"], "server_not_running");
    }

    #[test]
    fn test_respond_generic_error() {
        let result: Result<String> = Err(CliError::Searxng("bad".to_string()));
        let (status, body) = respond(result);
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        let val: serde_json::Value = serde_json::to_value(body.0).unwrap();
        assert_eq!(val["success"], false);
        assert!(val["data"].is_null());
        assert_eq!(val["error"]["code"], "searxng_error");
    }
}