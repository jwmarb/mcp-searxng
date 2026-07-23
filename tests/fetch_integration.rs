use std::time::Duration;

use searxng_cli::fetch::Fetcher;
use searxng_cli::retry::RetryClient;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_fetch_success() {
    let mock_server = MockServer::start().await;
    
    let html = "<!DOCTYPE html><html><head><title>Test Page</title></head><body><p>Content here</p></body></html>";
    
    Mock::given(method("GET")).and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html)
            .insert_header("Content-Type", "text/html"))
        .mount(&mock_server)
        .await;
    
    let fetcher = Fetcher::new(RetryClient::new(&Default::default()));
    let params = searxng_cli::fetch::FetchParams {
        url: format!("http://{}", mock_server.address()),
        max_chars: None,
        timeout: None,
        render_mode: searxng_cli::fetch::RenderMode::Lightweight,
    };
    
    let result = fetcher.fetch(&params).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert_eq!(response.title, "Test Page");
    assert!(response.content.contains("Content"));
}

#[tokio::test]
async fn test_fetch_max_chars() {
    let mock_server = MockServer::start().await;
    
    let long_content = "A".repeat(1000);
    let html = format!("<!DOCTYPE html><html><body>{}</body></html>", long_content);
    
    Mock::given(method("GET")).and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&mock_server)
        .await;
    
    let fetcher = Fetcher::new(RetryClient::new(&Default::default()));
    let params = searxng_cli::fetch::FetchParams {
        url: format!("http://{}", mock_server.address()),
        max_chars: Some(100),
        timeout: None,
        render_mode: searxng_cli::fetch::RenderMode::Lightweight,
    };
    
    let result = fetcher.fetch(&params).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert!(response.content.len() <= 105);
}

#[tokio::test]
async fn test_fetch_empty_body() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("GET")).and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .mount(&mock_server)
        .await;
    
    let fetcher = Fetcher::new(RetryClient::new(&Default::default()));
    let params = searxng_cli::fetch::FetchParams {
        url: format!("http://{}", mock_server.address()),
        max_chars: None,
        timeout: None,
        render_mode: searxng_cli::fetch::RenderMode::Lightweight,
    };
    
    let result = fetcher.fetch(&params).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert_eq!(response.title, "");
    assert_eq!(response.content, "");
    assert_eq!(response.content_length, 0);
}

#[tokio::test]
async fn test_fetch_no_title_tag() {
    let mock_server = MockServer::start().await;
    
    let html = "<html><body>No title here</body></html>";
    
    Mock::given(method("GET")).and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&mock_server)
        .await;
    
    let fetcher = Fetcher::new(RetryClient::new(&Default::default()));
    let params = searxng_cli::fetch::FetchParams {
        url: format!("http://{}", mock_server.address()),
        max_chars: None,
        timeout: None,
        render_mode: searxng_cli::fetch::RenderMode::Lightweight,
    };
    
    let result = fetcher.fetch(&params).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert_eq!(response.title, "");
    assert!(response.content_length > 0);
}

#[tokio::test]
async fn test_fetch_content_length_accuracy() {
    let mock_server = MockServer::start().await;

    let body_content = "B".repeat(1000);
    let html = format!("<!DOCTYPE html><html><body>{}</body></html>", body_content);

    Mock::given(method("GET")).and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&mock_server)
        .await;

    let fetcher = Fetcher::new(RetryClient::new(&Default::default()));
    let params = searxng_cli::fetch::FetchParams {
        url: format!("http://{}", mock_server.address()),
        max_chars: Some(100),
        timeout: None,
        render_mode: searxng_cli::fetch::RenderMode::Lightweight,
    };

    let result = fetcher.fetch(&params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert!(response.content_length <= 105);
    assert_eq!(response.content_length, response.content.len());
}

#[tokio::test]
async fn test_fetch_server_404() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET")).and(path("/"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&mock_server)
        .await;

    let fetcher = Fetcher::new(RetryClient::new(&Default::default()));
    let params = searxng_cli::fetch::FetchParams {
        url: format!("http://{}", mock_server.address()),
        max_chars: None,
        timeout: None,
        render_mode: searxng_cli::fetch::RenderMode::Lightweight,
    };

    let result = fetcher.fetch(&params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.status_code, 404);
}

#[tokio::test]
async fn test_fetch_server_500() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET")).and(path("/"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock_server)
        .await;

    let fetcher = Fetcher::new(RetryClient::new(&Default::default()));
    let params = searxng_cli::fetch::FetchParams {
        url: format!("http://{}", mock_server.address()),
        max_chars: None,
        timeout: None,
        render_mode: searxng_cli::fetch::RenderMode::Lightweight,
    };

    let result = fetcher.fetch(&params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.status_code, 500);
}

#[tokio::test]
async fn test_fetch_timeout() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET")).and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(60)))
        .mount(&mock_server)
        .await;

    let fetcher = Fetcher::new(RetryClient::new(&Default::default()));
    let params = searxng_cli::fetch::FetchParams {
        url: format!("http://{}", mock_server.address()),
        max_chars: None,
        timeout: Some(1),
        render_mode: searxng_cli::fetch::RenderMode::Lightweight,
    };

    let result = fetcher.fetch(&params).await;
    assert!(result.is_err());
}
