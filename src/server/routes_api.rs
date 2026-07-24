use std::sync::Arc;
use std::time::Instant;

use axum::{extract::{Query, Extension}, Json, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use crate::config::Config;
use crate::response::ApiResponse;
use crate::search::{Search, SearchParams, OutputFormat as SearchOutputFormat, SearchResponse};
use crate::fetch::{Fetcher, FetchParams, RenderMode};
use crate::retry::RetryClient;
use crate::browser::pool::PoolStatus;

use crate::server::routes::{respond, bad_request_response};

#[derive(Deserialize)]
pub(crate) struct SearchQueryParams {
    #[serde(default)]
    pub(crate) query: Option<String>,
    #[serde(default)]
    pub(crate) categories: Option<String>,
    #[serde(default)]
    pub(crate) language: Option<String>,
    #[serde(default)]
    pub(crate) time_range: Option<String>,
    #[serde(default)]
    pub(crate) safesearch: Option<u8>,
    #[serde(default)]
    pub(crate) page: Option<u32>,
    #[serde(default)]
    pub(crate) max_results: Option<usize>,
}

#[derive(Deserialize)]
pub(crate) struct FetchRequestBody {
    #[serde(default)]
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) max_chars: Option<usize>,
    #[serde(default)]
    pub(crate) timeout: Option<u64>,
    #[serde(default)]
    pub(crate) render: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    pub(crate) healthy: bool,
    pub(crate) searxng_reachable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pool_status: Option<PoolStatus>,
}

pub(crate) async fn health_check(
    search: Option<Extension<Arc<Search>>>,
    session_manager: Option<Extension<crate::server::session::SessionManager>>,
) -> impl IntoResponse {
    let started_at = Instant::now();

    let searxng_reachable = if let Some(Extension(search)) = &search {
        search.ping().await
    } else {
        false
    };

    let pool_status = if let Some(Extension(_manager)) = &session_manager {
        // Pool status requires the full SessionManager; we can't easily access it
        // from the health endpoint when only the Extension is available.
        None
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

pub(crate) async fn api_search(
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

pub(crate) async fn api_fetch(
    Extension(config): Extension<Arc<Config>>,
    Json(body): Json<FetchRequestBody>,
) -> (StatusCode, Json<ApiResponse<crate::fetch::FetchResponse>>) {
    let url = match &body.url {
        Some(u) if !u.trim().is_empty() => u.clone(),
        _ => return bad_request_response("Missing required parameter: url"),
    };

    if url::Url::parse(&url).is_err() {
        return bad_request_response(&format!("Invalid URL: {url}"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CliError;
    use serde_json::json;

    #[test]
    fn test_search_query_params_defaults() {
        let data = json!({});
        let params: SearchQueryParams = serde_json::from_value(data).unwrap();
        assert_eq!(params.query, None);
        assert_eq!(params.categories, None);
        assert_eq!(params.language, None);
        assert_eq!(params.safesearch, None);
        assert_eq!(params.page, None);
        assert_eq!(params.max_results, None);
    }

    #[test]
    fn test_search_query_params_full() {
        let data = json!({
            "query": "test",
            "categories": "general",
            "language": "en",
            "time_range": "day",
            "safesearch": 1,
            "page": 2,
            "max_results": 20
        });
        let params: SearchQueryParams = serde_json::from_value(data).unwrap();
        assert_eq!(params.query, Some("test".to_string()));
        assert_eq!(params.categories, Some("general".to_string()));
        assert_eq!(params.language, Some("en".to_string()));
        assert_eq!(params.time_range, Some("day".to_string()));
        assert_eq!(params.safesearch, Some(1));
        assert_eq!(params.page, Some(2));
        assert_eq!(params.max_results, Some(20));
    }

    #[test]
    fn test_fetch_request_body_defaults() {
        let data = json!({});
        let body: FetchRequestBody = serde_json::from_value(data).unwrap();
        assert_eq!(body.url, None);
        assert_eq!(body.max_chars, None);
        assert_eq!(body.timeout, None);
        assert_eq!(body.render, None);
    }

    #[test]
    fn test_fetch_request_body_full() {
        let data = json!({
            "url": "https://example.com",
            "max_chars": 1000,
            "timeout": 30,
            "render": true
        });
        let body: FetchRequestBody = serde_json::from_value(data).unwrap();
        assert_eq!(body.url, Some("https://example.com".to_string()));
        assert_eq!(body.max_chars, Some(1000));
        assert_eq!(body.timeout, Some(30));
        assert_eq!(body.render, Some(true));
    }

    #[test]
    fn test_respond_success() {
        let result: crate::error::Result<String> = Ok("test".to_string());
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
        let result: crate::error::Result<String> = Err(CliError::SessionNotFound("x".to_string()));
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
        let result: crate::error::Result<String> = Err(CliError::SessionRequired);
        let (status, body) = respond(result);
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let val: serde_json::Value = serde_json::to_value(body.0).unwrap();
        assert_eq!(val["success"], false);
        assert!(val["data"].is_null());
        assert_eq!(val["error"]["code"], "session_required");
    }

    #[test]
    fn test_respond_server_not_running() {
        let result: crate::error::Result<String> = Err(CliError::ServerNotRunning);
        let (status, body) = respond(result);
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let val: serde_json::Value = serde_json::to_value(body.0).unwrap();
        assert_eq!(val["success"], false);
        assert!(val["data"].is_null());
        assert_eq!(val["error"]["code"], "server_not_running");
    }

    #[test]
    fn test_respond_generic_error() {
        let result: crate::error::Result<String> = Err(CliError::Searxng("bad".to_string()));
        let (status, body) = respond(result);
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        let val: serde_json::Value = serde_json::to_value(body.0).unwrap();
        assert_eq!(val["success"], false);
        assert!(val["data"].is_null());
        assert_eq!(val["error"]["code"], "searxng_error");
    }
}