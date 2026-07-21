use searxng_cli::fetch::Fetcher;
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
    
    let fetcher = Fetcher::new();
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
    
    let fetcher = Fetcher::new();
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
    
    let fetcher = Fetcher::new();
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
}

#[tokio::test]
async fn test_fetch_no_title_tag() {
    let mock_server = MockServer::start().await;
    
    let html = "<html><body>No title here</body></html>";
    
    Mock::given(method("GET")).and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .mount(&mock_server)
        .await;
    
    let fetcher = Fetcher::new();
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
}
