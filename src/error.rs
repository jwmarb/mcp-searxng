use axum::http::StatusCode;
use axum::response::IntoResponse;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("SearXNG API error: {0}")]
    Searxng(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

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
}

impl IntoResponse for CliError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match &self {
            CliError::SessionNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            CliError::ServerNotRunning => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            CliError::Http(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, CliError>;

unsafe impl Send for CliError {}
unsafe impl Sync for CliError {}