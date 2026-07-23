use std::time::Duration;

use reqwest::Response;
use serde::{Deserialize, Serialize};

/// Configuration for HTTP retry behavior with exponential backoff.
#[derive(Debug, Clone, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    #[serde(default = "default_base_delay_ms")]
    pub base_delay_ms: u64,

    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_max_retries() -> u32 {
    3
}

fn default_base_delay_ms() -> u64 {
    200
}

fn default_timeout_secs() -> u64 {
    15
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            base_delay_ms: default_base_delay_ms(),
            timeout_secs: default_timeout_secs(),
        }
    }
}

impl RetryConfig {
    /// Compute delay for a given attempt (0-based).
    /// Uses exponential backoff: base_delay * 2^attempt + random_jitter(0..base_delay).
    fn compute_delay(&self, attempt: u32) -> Duration {
        let exp = self.base_delay_ms * 2u64.pow(attempt);
        let jitter = fastrand::u64(0..self.base_delay_ms);
        Duration::from_millis(exp + jitter)
    }
}

/// HTTP client with built-in retry logic and exponential backoff.
///
/// Wraps a `reqwest::Client` and automatically retries on:
/// - Connection errors (`is_connect()`)
/// - Timeout errors (`is_timeout()`)
/// - HTTP 5xx server errors
///
/// Does NOT retry on:
/// - HTTP 4xx client errors
/// - DNS resolution failures
/// - Redirect loops
#[derive(Debug, Clone)]
pub struct RetryClient {
    config: RetryConfig,
    inner: reqwest::Client,
}

impl RetryClient {
    pub fn new(config: &RetryConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("reqwest client builder");
        Self {
            config: config.clone(),
            inner: client,
        }
    }

    fn should_retry(&self, err: &reqwest::Error) -> bool {
        err.is_connect() || err.is_timeout()
    }

    /// Perform a GET request with automatic retry.
    pub async fn get(&self, url: &str) -> reqwest::Result<Response> {
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let delay = self.config.compute_delay(attempt - 1);
                tokio::time::sleep(delay).await;
            }

            match self.inner.get(url).send().await {
                Ok(resp) if resp.status().is_server_error() && attempt < self.config.max_retries => {
                    // 5xx — retry (unless we've exhausted attempts)
                    continue;
                }
                Ok(resp) => return Ok(resp),
                Err(e) if self.should_retry(&e) && attempt < self.config.max_retries => {
                    // Connection/timeout — retry
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        // Should be unreachable due to the loop structure
        unreachable!("retry loop should always return");
    }

    /// Perform a POST request with JSON body and automatic retry.
    pub async fn post_json(
        &self,
        url: &str,
        body: &impl Serialize,
    ) -> reqwest::Result<Response> {
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                let delay = self.config.compute_delay(attempt - 1);
                tokio::time::sleep(delay).await;
            }

            match self.inner.post(url).json(body).send().await {
                Ok(resp) if resp.status().is_server_error() && attempt < self.config.max_retries => {
                    continue;
                }
                Ok(resp) => return Ok(resp),
                Err(e) if self.should_retry(&e) && attempt < self.config.max_retries => {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        unreachable!("retry loop should always return");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};

    // ── RetryConfig tests ──────────────────────────────────────────────────

    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay_ms, 200);
        assert_eq!(config.timeout_secs, 15);
    }

    #[test]
    fn test_retry_config_deserialize_empty() {
        let yaml = "{}";
        let config: RetryConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay_ms, 200);
        assert_eq!(config.timeout_secs, 15);
    }

    #[test]
    fn test_retry_config_deserialize_custom() {
        let yaml = "max_retries: 5\nbase_delay_ms: 500\ntimeout_secs: 30";
        let config: RetryConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.base_delay_ms, 500);
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_retry_config_deserialize_partial() {
        let yaml = "max_retries: 2";
        let config: RetryConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.base_delay_ms, 200); // default
        assert_eq!(config.timeout_secs, 15);    // default
    }

    #[test]
    fn test_retry_config_compute_delay_zero_based() {
        let config = RetryConfig::default();
        // First attempt (0): delay = 200 * 2^0 + jitter = 200 + jitter(0..200)
        let delay = config.compute_delay(0);
        assert!(delay.as_millis() >= 200);
        assert!(delay.as_millis() < 400); // 200 + max jitter (199)

        // Second attempt (1): delay = 200 * 2^1 + jitter = 400 + jitter(0..200)
        let delay = config.compute_delay(1);
        assert!(delay.as_millis() >= 400);
        assert!(delay.as_millis() < 600);
    }

    #[test]
    fn test_retry_config_compute_delay_respects_jitter_range() {
        let config = RetryConfig {
            base_delay_ms: 100,
            ..Default::default()
        };
        // Run multiple times to ensure jitter stays within bounds
        for _ in 0..100 {
            let delay = config.compute_delay(0);
            let ms = delay.as_millis() as u64;
            assert!(ms >= 100, "delay {ms} below base");
            assert!(ms < 200, "delay {ms} above base + jitter max");
        }
    }

    // ── Helper: custom wiremock Respond for sequence testing ───────────────

    struct SequenceRespond {
        counter: Arc<AtomicU32>,
        first_body: String,
        first_status: u16,
        second_body: String,
        second_status: u16,
    }

    impl wiremock::Respond for SequenceRespond {
        fn respond(&self, _request: &wiremock::Request) -> ResponseTemplate {
            let count = self.counter.fetch_add(1, Ordering::SeqCst);
            if count == 0 {
                ResponseTemplate::new(self.first_status)
                    .set_body_string(self.first_body.clone())
            } else {
                ResponseTemplate::new(self.second_status)
                    .set_body_string(self.second_body.clone())
            }
        }
    }

    // ── RetryClient integration tests with wiremock ────────────────────────

    #[tokio::test]
    async fn test_retry_client_retries_on_500() {
        let mock_server = MockServer::start().await;

        // Expect 4 requests total: 1 initial + 3 retries (max_retries=3)
        Mock::given(method("GET"))
            .and(path("/retry-500"))
            .respond_with(ResponseTemplate::new(500))
            .expect(4)
            .mount(&mock_server)
            .await;

        let config = RetryConfig::default();
        let client = RetryClient::new(&config);
        let url = format!("http://{}/retry-500", mock_server.address());

        let result = client.get(&url).await;
        // All 4 attempts returned 500, so final result is still a 500 response
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status().as_u16(), 500);
    }

    #[tokio::test]
    async fn test_retry_client_no_retry_on_404() {
        let mock_server = MockServer::start().await;

        // Expect exactly 1 request — no retries on 404
        Mock::given(method("GET"))
            .and(path("/not-found"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = RetryConfig::default();
        let client = RetryClient::new(&config);
        let url = format!("http://{}/not-found", mock_server.address());

        let result = client.get(&url).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status().as_u16(), 404);
    }

    #[tokio::test]
    async fn test_retry_client_succeeds_on_second_attempt() {
        let mock_server = MockServer::start().await;

        let counter = Arc::new(AtomicU32::new(0));

        Mock::given(method("GET"))
            .and(path("/retry-then-ok"))
            .respond_with(SequenceRespond {
                counter: counter.clone(),
                first_body: "error".to_string(),
                first_status: 500,
                second_body: "ok".to_string(),
                second_status: 200,
            })
            .expect(2) // 1 initial + 1 retry
            .mount(&mock_server)
            .await;

        let config = RetryConfig::default();
        let client = RetryClient::new(&config);
        let url = format!("http://{}/retry-then-ok", mock_server.address());

        let result = client.get(&url).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
    }

    #[tokio::test]
    async fn test_retry_client_respects_max_retries_count() {
        let mock_server = MockServer::start().await;

        // max_retries=1 → total of 2 requests (1 initial + 1 retry)
        Mock::given(method("GET"))
            .and(path("/max-retries"))
            .respond_with(ResponseTemplate::new(500))
            .expect(2)
            .mount(&mock_server)
            .await;

        let config = RetryConfig {
            max_retries: 1,
            ..Default::default()
        };
        let client = RetryClient::new(&config);
        let url = format!("http://{}/max-retries", mock_server.address());

        let result = client.get(&url).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status().as_u16(), 500);
    }

    #[tokio::test]
    async fn test_retry_client_no_retry_on_301() {
        let mock_server = MockServer::start().await;

        // 3xx redirects should not be retried
        Mock::given(method("GET"))
            .and(path("/redirect"))
            .respond_with(ResponseTemplate::new(301))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = RetryConfig::default();
        let client = RetryClient::new(&config);
        let url = format!("http://{}/redirect", mock_server.address());

        let result = client.get(&url).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status().as_u16(), 301);
    }

    #[tokio::test]
    async fn test_retry_client_no_retry_on_403() {
        let mock_server = MockServer::start().await;

        // 403 is 4xx, should not be retried
        Mock::given(method("GET"))
            .and(path("/forbidden"))
            .respond_with(ResponseTemplate::new(403))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = RetryConfig::default();
        let client = RetryClient::new(&config);
        let url = format!("http://{}/forbidden", mock_server.address());

        let result = client.get(&url).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status().as_u16(), 403);
    }

    #[tokio::test]
    async fn test_retry_client_succeeds_first_try() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(200).set_body_string("success"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = RetryConfig::default();
        let client = RetryClient::new(&config);
        let url = format!("http://{}/ok", mock_server.address());

        let result = client.get(&url).await;
        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
    }

    #[tokio::test]
    async fn test_retry_client_post_json_retries_on_500() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
            .expect(4)
            .mount(&mock_server)
            .await;

        let config = RetryConfig::default();
        let client = RetryClient::new(&config);
        let url = format!("http://{}/api", mock_server.address());

        let body = serde_json::json!({"key": "value"});
        let result = client.post_json(&url, &body).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status().as_u16(), 500);
    }

    #[tokio::test]
    async fn test_retry_client_post_json_no_retry_on_400() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api"))
            .respond_with(ResponseTemplate::new(400))
            .expect(1)
            .mount(&mock_server)
            .await;

        let config = RetryConfig::default();
        let client = RetryClient::new(&config);
        let url = format!("http://{}/api", mock_server.address());

        let body = serde_json::json!({"key": "value"});
        let result = client.post_json(&url, &body).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status().as_u16(), 400);
    }

    #[tokio::test]
    async fn test_retry_client_holds_inner_client_timeout() {
        let config = RetryConfig {
            timeout_secs: 30,
            ..Default::default()
        };
        let client = RetryClient::new(&config);
        // We can't easily inspect the inner timeout, but we can verify
        // the client was constructed without panicking
        assert_eq!(client.config.timeout_secs, 30);
    }
}
