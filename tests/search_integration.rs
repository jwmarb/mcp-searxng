use searxng_cli::search::{Search, SearchParams, OutputFormat};
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, query_param};

#[tokio::test]
async fn test_search_success() {
    let mock_server = MockServer::start().await;
    
    let body = r#"{"results": [{"title": "Test Result", "url": "https://example.com", "content": "This is a test result content."}]}"#;
    
    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock_server)
        .await;
    
    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
    };
    
    let search = Search::new(&config);
    let params = SearchParams {
        query: "test".to_string(),
        categories: None,
        language: None,
        time_range: None,
        safesearch: None,
        page: None,
        max_results: None,
        format: OutputFormat::Json,
    };
    
    let result = search.search(&params).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].title, "Test Result");
    assert_eq!(response.results[0].url, "https://example.com");
}

#[tokio::test]
async fn test_search_empty_results() {
    let mock_server = MockServer::start().await;
    
    let body = r#"{"results": []}"#;
    
    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock_server)
        .await;
    
    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
    };
    
    let search = Search::new(&config);
    let params = SearchParams {
        query: "empty".to_string(),
        categories: None,
        language: None,
        time_range: None,
        safesearch: None,
        page: None,
        max_results: None,
        format: OutputFormat::Json,
    };
    
    let result = search.search(&params).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert_eq!(response.results.len(), 0);
}

#[tokio::test]
async fn test_search_server_error() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;
    
    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
    };
    
    let search = Search::new(&config);
    let params = SearchParams {
        query: "error".to_string(),
        categories: None,
        language: None,
        time_range: None,
        safesearch: None,
        page: None,
        max_results: None,
        format: OutputFormat::Json,
    };
    
    let result = search.search(&params).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_search_malformed_json() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&mock_server)
        .await;
    
    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
    };
    
    let search = Search::new(&config);
    let params = SearchParams {
        query: "malformed".to_string(),
        categories: None,
        language: None,
        time_range: None,
        safesearch: None,
        page: None,
        max_results: None,
        format: OutputFormat::Json,
    };
    
    let result = search.search(&params).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_search_with_categories() {
    let mock_server = MockServer::start().await;
    
    let body = r#"{"results": [{"title": "News Result", "url": "https://news.com", "content": "Latest news"}]}"#;
    
    Mock::given(method("GET")).and(path("/search")).and(query_param("categories", "news"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock_server)
        .await;
    
    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
    };
    
    let search = Search::new(&config);
    let params = SearchParams {
        query: "news".to_string(),
        categories: Some("news".to_string()),
        language: None,
        time_range: None,
        safesearch: None,
        page: None,
        max_results: None,
        format: OutputFormat::Json,
    };
    
    let result = search.search(&params).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    assert_eq!(response.results[0].title, "News Result");
}

#[tokio::test]
async fn test_search_format_response_text() {
    let mock_server = MockServer::start().await;
    
    let body = r#"{"results": [{"title": "Test", "url": "https://test.com", "content": "Content here"}]}"#;
    
    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock_server)
        .await;
    
    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
    };
    
    let search = Search::new(&config);
    let params = SearchParams {
        query: "test".to_string(),
        categories: None,
        language: None,
        time_range: None,
        safesearch: None,
        page: None,
        max_results: None,
        format: OutputFormat::Text,
    };
    
    let result = search.search(&params).await;
    assert!(result.is_ok());
    
    let response = result.unwrap();
    let formatted = Search::format_response(&response, OutputFormat::Text);
    assert!(formatted.contains("Test"));
    assert!(formatted.contains("https://test.com"));
}
