use serde::Serialize;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::error::ApiError;
use crate::time::format_timestamp;

#[derive(Debug, Clone, Serialize)]
pub struct ResponseMetadata {
    pub duration_ms: u64,
    pub timestamp: String,
}

impl ResponseMetadata {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration_ms: duration.as_millis() as u64,
            timestamp: format_timestamp(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO)),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
    pub metadata: ResponseMetadata,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T, started_at: Instant) -> Self {
        let elapsed = started_at.elapsed();
        Self {
            success: true,
            data: Some(data),
            error: None,
            metadata: ResponseMetadata::new(elapsed),
        }
    }

    pub fn error(api_error: ApiError, started_at: Instant) -> Self {
        let elapsed = started_at.elapsed();
        Self {
            success: false,
            data: None,
            error: Some(api_error),
            metadata: ResponseMetadata::new(elapsed),
        }
    }

    pub fn from_cli_error(error: &crate::error::CliError, started_at: Instant) -> Self {
        Self::error(error.to_api_error(), started_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_response_serializes() {
        let started_at = Instant::now();
        let resp: ApiResponse<String> = ApiResponse::success("hello".to_string(), started_at);

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"], "hello");
        assert!(parsed["error"].is_null());
        assert!(parsed["metadata"]["duration_ms"].is_number());
        assert!(parsed["metadata"]["timestamp"].is_string());
        assert!(parsed["metadata"]["timestamp"].as_str().unwrap().ends_with('Z'));
    }

    #[test]
    fn test_error_response_serializes() {
        let started_at = Instant::now();
        let api_err = ApiError {
            code: "test_error".to_string(),
            message: "something went wrong".to_string(),
            retryable: false,
            hint: None,
        };
        let resp: ApiResponse<String> = ApiResponse::error(api_err, started_at);

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed["data"].is_null());
        assert!(parsed["error"].is_object());
        assert_eq!(parsed["error"]["code"], "test_error");
        assert_eq!(parsed["error"]["message"], "something went wrong");
        assert!(parsed["metadata"]["duration_ms"].is_number());
        assert!(parsed["metadata"]["timestamp"].is_string());
    }

    #[test]
    fn test_metadata_new() {
        let meta = ResponseMetadata::new(Duration::from_millis(150));
        assert_eq!(meta.duration_ms, 150);
        assert!(meta.timestamp.contains('T'));
        assert!(meta.timestamp.ends_with('Z'));
    }

    #[test]
    fn test_response_clone() {
        let started_at = Instant::now();
        let resp: ApiResponse<String> = ApiResponse::success("test".to_string(), started_at);
        let cloned = resp.clone();
        let json_orig = serde_json::to_string(&resp).unwrap();
        let json_clone = serde_json::to_string(&cloned).unwrap();
        assert_eq!(json_orig, json_clone);
    }

    #[test]
    fn test_success_response_without_data_serializes() {
        let started_at = Instant::now();
        let resp: ApiResponse<()> = ApiResponse::success((), started_at);

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed["error"].is_null());
    }

    #[test]
    fn test_response_from_cli_error() {
        use crate::error::CliError;
        let started_at = Instant::now();
        let err = CliError::SessionNotFound("abc".to_string());
        let resp: ApiResponse<()> = ApiResponse::from_cli_error(&err, started_at);

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed["data"].is_null());
        assert_eq!(parsed["error"]["code"], "session_not_found");
    }
}