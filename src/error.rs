use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use thiserror::Error;

use crate::response::ApiResponse;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorCode {
    Success,
    Client,    // exit code 1
    Server,    // exit code 2
    Timeout,   // exit code 3
    Session,   // exit code 4
}

impl ErrorCode {
    pub fn exit_code(&self) -> i32 {
        match self {
            ErrorCode::Success => 0,
            ErrorCode::Client => 1,
            ErrorCode::Server => 2,
            ErrorCode::Timeout => 3,
            ErrorCode::Session => 4,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("SearXNG API error: {0}")]
    Searxng(String),

    #[error("HTTP error: {0}")]
    Http(Arc<reqwest::Error>),

    #[error("Browser error: {0}")]
    Browser(String),

    #[error("Session '{0}' not found")]
    SessionNotFound(String),

    #[error("Server is not running")]
    ServerNotRunning,

    #[error("Config error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    #[error("Session ID is required. Use --session <ID>")]
    SessionRequired,
}

impl From<reqwest::Error> for CliError {
    fn from(e: reqwest::Error) -> Self {
        CliError::Http(Arc::new(e))
    }
}

impl IntoResponse for CliError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            CliError::SessionNotFound(_) => StatusCode::NOT_FOUND,
            CliError::SessionRequired => StatusCode::BAD_REQUEST,
            CliError::ServerNotRunning => StatusCode::SERVICE_UNAVAILABLE,
            CliError::Http(_) => StatusCode::BAD_GATEWAY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let api_error = self.to_api_error();
        let response = ApiResponse::<()>::error(api_error, std::time::Instant::now());
        (status, Json(response)).into_response()
    }
}

impl CliError {
    pub fn error_code(&self) -> ErrorCode {
        match self {
            CliError::Searxng(_) => ErrorCode::Server,
            CliError::Http(e) => {
                if e.is_timeout() {
                    ErrorCode::Timeout
                } else {
                    ErrorCode::Server
                }
            }
            CliError::Browser(_) => ErrorCode::Server,
            CliError::SessionNotFound(_) => ErrorCode::Session,
            CliError::SessionRequired => ErrorCode::Session,
            CliError::ServerNotRunning => ErrorCode::Server,
            CliError::Config(_) => ErrorCode::Client,
            CliError::Io(_) => ErrorCode::Client,
            CliError::Json(_) => ErrorCode::Client,
            CliError::Url(_) => ErrorCode::Client,
        }
    }

    pub fn to_api_error(&self) -> ApiError {
        ApiError {
            code: self.api_code().to_string(),
            message: self.to_string(),
            retryable: self.is_retryable(),
            hint: self.api_hint(),
        }
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            CliError::Http(e) => e.is_timeout(),
            _ => false,
        }
    }

    fn api_code(&self) -> &str {
        match self {
            CliError::Searxng(_) => "searxng_error",
            CliError::Http(e) => {
                if e.is_timeout() {
                    "http_timeout"
                } else {
                    "http_error"
                }
            }
            CliError::Browser(_) => "browser_error",
            CliError::SessionNotFound(_) => "session_not_found",
            CliError::SessionRequired => "session_required",
            CliError::ServerNotRunning => "server_not_running",
            CliError::Config(_) => "config_error",
            CliError::Io(_) => "io_error",
            CliError::Json(_) => "json_error",
            CliError::Url(_) => "url_error",
        }
    }

    fn api_hint(&self) -> Option<String> {
        match self {
            CliError::SessionNotFound(_) => {
                Some("Create a new session with navigate --session <id>".to_string())
            }
            CliError::SessionRequired => {
                Some("Provide a session ID with --session <id>".to_string())
            }
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, CliError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_string_searxng() {
        let e = CliError::Searxng("bad query".into());
        assert_eq!(e.to_string(), "SearXNG API error: bad query");
    }

    #[test]
    fn test_to_string_browser() {
        let e = CliError::Browser("timeout".into());
        assert_eq!(e.to_string(), "Browser error: timeout");
    }

    #[test]
    fn test_to_string_session_not_found() {
        let e = CliError::SessionNotFound("abc123".into());
        assert_eq!(e.to_string(), "Session 'abc123' not found");
    }

    #[test]
    fn test_to_string_server_not_running() {
        let e = CliError::ServerNotRunning;
        assert_eq!(e.to_string(), "Server is not running");
    }

    #[test]
    fn test_to_string_config() {
        let e = CliError::Config("missing key".into());
        assert_eq!(e.to_string(), "Config error: missing key");
    }

    #[test]
    fn test_into_response_session_not_found() {
        let e = CliError::SessionNotFound("x".into());
        let resp = e.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_into_response_server_not_running() {
        let e = CliError::ServerNotRunning;
        let resp = e.into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_into_response_http() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let reqwest_err = rt.block_on(async {
            reqwest::get("http://127.0.0.1:1").await.unwrap_err()
        });
        let e = CliError::Http(Arc::new(reqwest_err));
        let resp = e.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_into_response_default_internal() {
        let e = CliError::Searxng("x".into());
        let resp = e.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no file");
        let e: CliError = io_err.into();
        assert!(matches!(e, CliError::Io(_)));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
        let e: CliError = json_err.into();
        assert!(matches!(e, CliError::Json(_)));
    }

    #[test]
    fn test_from_url_parse_error() {
        let url_err = "not a url".parse::<url::Url>().unwrap_err();
        let e: CliError = url_err.into();
        assert!(matches!(e, CliError::Url(_)));
    }

    #[test]
    fn test_error_code_searxng() {
        assert_eq!(CliError::Searxng("x".into()).error_code(), ErrorCode::Server);
    }

    #[test]
    fn test_error_code_browser() {
        assert_eq!(CliError::Browser("x".into()).error_code(), ErrorCode::Server);
    }

    #[test]
    fn test_error_code_session_not_found() {
        assert_eq!(CliError::SessionNotFound("x".into()).error_code(), ErrorCode::Session);
    }

    #[test]
    fn test_error_code_server_not_running() {
        assert_eq!(CliError::ServerNotRunning.error_code(), ErrorCode::Server);
    }

    #[test]
    fn test_error_code_config() {
        assert_eq!(CliError::Config("x".into()).error_code(), ErrorCode::Client);
    }

    #[test]
    fn test_error_code_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no file");
        assert_eq!(CliError::from(io_err).error_code(), ErrorCode::Client);
    }

    #[test]
    fn test_error_code_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
        assert_eq!(CliError::from(json_err).error_code(), ErrorCode::Client);
    }

    #[test]
    fn test_error_code_url() {
        let url_err = "not a url".parse::<url::Url>().unwrap_err();
        assert_eq!(CliError::from(url_err).error_code(), ErrorCode::Client);
    }

    #[test]
    fn test_error_code_http_timeout() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(1))
            .build()
            .unwrap();
        let reqwest_err = rt.block_on(async {
            client.get("http://10.255.255.1:9999").send().await.unwrap_err()
        });
        assert!(reqwest_err.is_timeout(), "expected timeout error, got: {}", reqwest_err);
        assert_eq!(CliError::Http(Arc::new(reqwest_err)).error_code(), ErrorCode::Timeout);
    }

    #[test]
    fn test_error_code_http_non_timeout() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let reqwest_err = rt.block_on(async { reqwest::get("http://127.0.0.1:1").await.unwrap_err() });
        assert_eq!(CliError::Http(Arc::new(reqwest_err)).error_code(), ErrorCode::Server);
    }

    #[test]
    fn test_to_string_session_required() {
        let e = CliError::SessionRequired;
        assert_eq!(e.to_string(), "Session ID is required. Use --session <ID>");
    }

    #[test]
    fn test_into_response_session_required() {
        let e = CliError::SessionRequired;
        let resp = e.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_error_code_session_required() {
        assert_eq!(CliError::SessionRequired.error_code(), ErrorCode::Session);
    }

    #[test]
    fn test_exit_code_values() {
        assert_eq!(ErrorCode::Success.exit_code(), 0);
        assert_eq!(ErrorCode::Client.exit_code(), 1);
        assert_eq!(ErrorCode::Server.exit_code(), 2);
        assert_eq!(ErrorCode::Timeout.exit_code(), 3);
        assert_eq!(ErrorCode::Session.exit_code(), 4);
    }

    #[test]
    fn test_api_error_creation_and_serialization() {
        let err = ApiError {
            code: "session_not_found".to_string(),
            message: "Session 'abc' not found".to_string(),
            retryable: false,
            hint: Some("Create a new session with navigate --session <id>".to_string()),
        };
        let json = serde_json::to_string(&err).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["code"], "session_not_found");
        assert_eq!(parsed["message"], "Session 'abc' not found");
        assert_eq!(parsed["retryable"], false);
        assert_eq!(parsed["hint"], "Create a new session with navigate --session <id>");
    }

    #[test]
    fn test_api_error_serialization_without_hint() {
        let err = ApiError {
            code: "timeout".to_string(),
            message: "Request timed out".to_string(),
            retryable: true,
            hint: None,
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(!json.contains("hint"));
    }

    #[test]
    fn test_cli_error_to_api_error_searxng() {
        let e = CliError::Searxng("bad query".into());
        let api = e.to_api_error();
        assert_eq!(api.code, "searxng_error");
        assert_eq!(api.retryable, false);
        assert_eq!(api.hint, None);
    }

    #[test]
    fn test_cli_error_to_api_error_session_not_found() {
        let e = CliError::SessionNotFound("abc".into());
        let api = e.to_api_error();
        assert_eq!(api.code, "session_not_found");
        assert_eq!(api.retryable, false);
    }

    #[test]
    fn test_cli_error_to_api_error_http_timeout() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(1))
            .build()
            .unwrap();
        let reqwest_err = rt.block_on(async {
            client.get("http://10.255.255.1:9999").send().await.unwrap_err()
        });
        let e = CliError::Http(Arc::new(reqwest_err));
        let api = e.to_api_error();
        assert_eq!(api.code, "http_timeout");
        assert_eq!(api.retryable, true);
    }

    #[test]
    fn test_cli_error_is_retryable() {
        assert!(!CliError::Searxng("x".into()).is_retryable());
        assert!(!CliError::SessionNotFound("x".into()).is_retryable());
        assert!(!CliError::Config("x".into()).is_retryable());
        assert!(!CliError::Browser("x".into()).is_retryable());
        assert!(!CliError::SessionRequired.is_retryable());
    }

    #[test]
    fn test_cli_error_is_retryable_timeout() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(1))
            .build()
            .unwrap();
        let reqwest_err = rt.block_on(async {
            client.get("http://10.255.255.1:9999").send().await.unwrap_err()
        });
        assert!(CliError::Http(Arc::new(reqwest_err)).is_retryable());
    }

    #[test]
    fn test_error_code_serialization() {
        assert_eq!(
            serde_json::to_string(&ErrorCode::Success).unwrap(),
            "\"success\""
        );
        assert_eq!(
            serde_json::to_string(&ErrorCode::Client).unwrap(),
            "\"client\""
        );
        assert_eq!(
            serde_json::to_string(&ErrorCode::Timeout).unwrap(),
            "\"timeout\""
        );
    }

    #[test]
    fn test_into_response_returns_json_api_response() {
        let e = CliError::SessionNotFound("abc".into());
        let resp = e.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
        assert!(content_type.contains("application/json"));
    }

    #[test]
    fn test_into_response_json_body_has_api_response_shape() {
        let e = CliError::SessionNotFound("abc".into());
        let resp = e.into_response();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let body_bytes = rt.block_on(async {
            let (_, body) = resp.into_parts();
            axum::body::to_bytes(body, usize::MAX).await.unwrap()
        });
        let parsed: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed["data"].is_null());
        assert!(parsed["error"].is_object());
        assert_eq!(parsed["error"]["code"], "session_not_found");
        assert!(parsed["error"]["message"].is_string());
        assert!(parsed["metadata"]["timestamp"].is_string());
        assert!(parsed["metadata"]["duration_ms"].is_number());
    }

    #[test]
    fn test_cli_error_is_send_sync() {
        fn _assert_send_sync<T: Send + Sync>() {}
        _assert_send_sync::<CliError>();
    }
}