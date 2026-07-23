use std::sync::Arc;
use std::net::SocketAddr;

use tokio::net::TcpListener;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

use searxng_cli::search::Search;
use searxng_cli::config::Config;
use searxng_cli::server::routes::create_api_router;

async fn spawn_test_app(search: Arc<Search>, config: Arc<Config>) -> SocketAddr {
    let app = create_api_router(search, config);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn test_config(mock_addr: &str) -> Config {
    Config {
        searxng_url: format!("http://{}", mock_addr),
        server_port: 18960,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
        retry: searxng_cli::retry::RetryConfig::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
    }
}

// ── Search endpoint tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_api_search_success() {
    let mock = MockServer::start().await;
    let body = r#"{"results":[{"title":"Test Result","url":"https://example.com","content":"Hello world"}]}"#;

    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock).await;

    let config = test_config(&mock.address().to_string());
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(config);
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/api/search?query=test", addr))
        .send().await.unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["results"][0]["title"], "Test Result");
    assert_eq!(json["data"]["results"][0]["url"], "https://example.com");
    assert!(json["error"].is_null());
    assert!(json["metadata"]["timestamp"].is_string());
    assert!(json["metadata"]["duration_ms"].is_number());
}

#[tokio::test]
async fn test_api_search_missing_query_returns_400() {
    let config = Arc::new(test_config("localhost:1"));
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(test_config("localhost:1"));
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/api/search", addr))
        .send().await.unwrap();

    assert_eq!(resp.status(), 400);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], false);
    assert!(json["data"].is_null());
    assert_eq!(json["error"]["code"], "bad_request");
    assert!(json["error"]["message"].as_str().unwrap().contains("query"));
}

#[tokio::test]
async fn test_api_search_empty_query_returns_400() {
    let config = Arc::new(test_config("localhost:1"));
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(test_config("localhost:1"));
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/api/search?query=", addr))
        .send().await.unwrap();

    assert_eq!(resp.status(), 400);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], false);
    assert_eq!(json["error"]["code"], "bad_request");
}

#[tokio::test]
async fn test_api_search_with_all_params() {
    let mock = MockServer::start().await;
    let body = r#"{"results":[{"title":"Full Params","url":"https://full.com","content":"All params used"}]}"#;

    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock).await;

    let config = test_config(&mock.address().to_string());
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(config);
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://{}/api/search?query=test&categories=news&language=en&time_range=week&safesearch=1&page=2&max_results=5",
            addr
        ))
        .send().await.unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["results"][0]["title"], "Full Params");
}

#[tokio::test]
async fn test_api_search_server_error() {
    let mock = MockServer::start().await;

    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock).await;

    let config = test_config(&mock.address().to_string());
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(config);
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/api/search?query=test", addr))
        .send().await.unwrap();

    assert_eq!(resp.status(), 500);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], false);
    assert!(json["data"].is_null());
    assert!(json["error"]["code"].is_string());
}

// ── Fetch endpoint tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_api_fetch_success() {
    let mock = MockServer::start().await;
    let html = "<!DOCTYPE html><html><head><title>Fetched Page</title></head><body><p>Important content here</p></body></html>";

    Mock::given(method("GET")).and(path("/page"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string(html)
            .insert_header("Content-Type", "text/html"))
        .mount(&mock).await;

    let config = test_config(&mock.address().to_string());
    // Fetch needs its own mock for the target URL
    let fetch_url = format!("http://{}/page", mock.address());
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(test_config("localhost:1")); // fetch doesn't use searxng_url
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/api/fetch", addr))
        .json(&serde_json::json!({"url": fetch_url}))
        .send().await.unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["title"], "Fetched Page");
    assert!(json["data"]["content"].as_str().unwrap().contains("Important content"));
    assert_eq!(json["data"]["status_code"], 200);
    assert!(json["error"].is_null());
}

#[tokio::test]
async fn test_api_fetch_missing_url_returns_400() {
    let config = Arc::new(test_config("localhost:1"));
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(test_config("localhost:1"));
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/api/fetch", addr))
        .json(&serde_json::json!({}))
        .send().await.unwrap();

    assert_eq!(resp.status(), 400);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], false);
    assert_eq!(json["error"]["code"], "bad_request");
    assert!(json["error"]["message"].as_str().unwrap().contains("url"));
}

#[tokio::test]
async fn test_api_fetch_empty_url_returns_400() {
    let config = Arc::new(test_config("localhost:1"));
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(test_config("localhost:1"));
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/api/fetch", addr))
        .json(&serde_json::json!({"url": ""}))
        .send().await.unwrap();

    assert_eq!(resp.status(), 400);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["error"]["code"], "bad_request");
}

#[tokio::test]
async fn test_api_fetch_invalid_url_returns_error() {
    let config = Arc::new(test_config("localhost:1"));
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(test_config("localhost:1"));
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/api/fetch", addr))
        .json(&serde_json::json!({"url": "not-a-valid-url-!!!@@@@"}))
        .send().await.unwrap();

    assert_eq!(resp.status(), 400);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], false);
    assert_eq!(json["error"]["code"], "bad_request");
    assert!(json["error"]["message"].as_str().unwrap().contains("Invalid URL"));
}

#[tokio::test]
async fn test_api_fetch_with_max_chars() {
    let mock = MockServer::start().await;
    let long_text = "X".repeat(2000);
    let html = format!("<!DOCTYPE html><html><body>{}</body></html>", long_text);

    Mock::given(method("GET")).and(path("/long"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_string(html)
            .insert_header("Content-Type", "text/html"))
        .mount(&mock).await;

    let config = test_config(&mock.address().to_string());
    let fetch_url = format!("http://{}/long", mock.address());
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(test_config("localhost:1"));
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/api/fetch", addr))
        .json(&serde_json::json!({"url": fetch_url, "max_chars": 100}))
        .send().await.unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
    let content = json["data"]["content"].as_str().unwrap();
    assert!(content.len() <= 105); // 100 + "..."
    assert!(json["data"]["content_length"].as_u64().unwrap() <= 105);
}

#[tokio::test]
async fn test_api_fetch_404_still_returns_data() {
    let mock = MockServer::start().await;

    Mock::given(method("GET")).and(path("/notfound"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&mock).await;

    let config = test_config(&mock.address().to_string());
    let fetch_url = format!("http://{}/notfound", mock.address());
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(test_config("localhost:1"));
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/api/fetch", addr))
        .json(&serde_json::json!({"url": fetch_url}))
        .send().await.unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["status_code"], 404);
}

#[tokio::test]
async fn test_api_health_still_works() {
    let config = Arc::new(test_config("localhost:1"));
    let search = Arc::new(Search::new(&config));
    let config = Arc::new(test_config("localhost:1"));
    let addr = spawn_test_app(search, config).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/api/health", addr))
        .send().await.unwrap();

    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["success"], true);
}
