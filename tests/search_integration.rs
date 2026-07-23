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
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
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
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
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
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
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
    assert!(result.unwrap_err().to_string().contains("status 500"));
}

#[tokio::test]
async fn test_search_with_all_params() {
    let mock_server = MockServer::start().await;

    let body = r#"{"results": [{"title": "All Params Result", "url": "https://example.com/all", "content": "Full params test"}]}"#;

    Mock::given(method("GET")).and(path("/search"))
        .and(query_param("categories", "news"))
        .and(query_param("language", "en"))
        .and(query_param("time_range", "week"))
        .and(query_param("safesearch", "1"))
        .and(query_param("pageno", "2"))
        .and(query_param("number_of_results", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock_server)
        .await;

    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
    };

    let search = Search::new(&config);
    let params = SearchParams {
        query: "all params".to_string(),
        categories: Some("news".to_string()),
        language: Some("en".to_string()),
        time_range: Some("week".to_string()),
        safesearch: Some(1),
        page: Some(2),
        max_results: Some(5),
        format: OutputFormat::Json,
    };

    let result = search.search(&params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].title, "All Params Result");
}

#[tokio::test]
async fn test_search_with_max_results() {
    let mock_server = MockServer::start().await;

    let body = r#"{"results": [{"title": "Max Results Test", "url": "https://example.com/max", "content": "Max results content"}]}"#;

    Mock::given(method("GET")).and(path("/search"))
        .and(query_param("number_of_results", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock_server)
        .await;

    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
    };

    let search = Search::new(&config);
    let params = SearchParams {
        query: "max results".to_string(),
        categories: None,
        language: None,
        time_range: None,
        safesearch: None,
        page: None,
        max_results: Some(10),
        format: OutputFormat::Json,
    };

    let result = search.search(&params).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].title, "Max Results Test");
}

#[tokio::test]
async fn test_search_special_chars_query() {
    let mock_server = MockServer::start().await;

    let body = r#"{"results": [{"title": "Special Chars", "url": "https://example.com/special", "content": "Special characters test"}]}"#;

    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&mock_server)
        .await;

    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
    };

    let search = Search::new(&config);
    let params = SearchParams {
        query: "hello world & test?".to_string(),
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
    assert_eq!(response.results[0].title, "Special Chars");
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
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
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
    let err = result.unwrap_err();
    assert!(err.to_string().contains("JSON") || err.to_string().contains("json") || err.to_string().contains("invalid"));
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
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
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
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
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

#[tokio::test]
async fn test_cache_hit_skips_http() {
    let mock_server = MockServer::start().await;

    let body = r#"{"results": [{"title": "Cached", "url": "https://cache.com", "content": "cache hit"}]}"#;

    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1) // only ONE HTTP request ever — second call must be cached
        .mount(&mock_server)
        .await;

    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
    };

    let search = Search::new(&config);
    let params = SearchParams {
        query: "cache-test".to_string(),
        categories: None,
        language: None,
        time_range: None,
        safesearch: None,
        page: None,
        max_results: None,
        format: OutputFormat::Json,
    };

    // First call: cache miss → hits wiremock
    let result1 = search.search(&params).await;
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap().results[0].title, "Cached");

    // Second call with same params: cache hit → NO additional HTTP request
    let result2 = search.search(&params).await;
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap().results[0].title, "Cached");
}

#[tokio::test]
async fn test_cache_miss_fetches() {
    let mock_server = MockServer::start().await;

    let body = r#"{"results": [{"title": "Fresh", "url": "https://fresh.com", "content": "fresh fetch"}]}"#;

    Mock::given(method("GET")).and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(2) // two different queries → two HTTP requests
        .mount(&mock_server)
        .await;

    let config = searxng_cli::config::Config {
        searxng_url: format!("http://{}", mock_server.address()),
        server_port: 8080,
        chrome_path: None,
        browser_server_url: "http://localhost:18960".to_string(),
        retry: Default::default(),
        max_sessions: 8,
        session_idle_timeout_secs: 600,
        cache: Default::default(),
    };

    let search = Search::new(&config);

    // Query A
    let params_a = SearchParams {
        query: "query-a".to_string(),
        categories: None,
        language: None,
        time_range: None,
        safesearch: None,
        page: None,
        max_results: None,
        format: OutputFormat::Json,
    };
    let result_a = search.search(&params_a).await;
    assert!(result_a.is_ok());

    // Query B (different)
    let params_b = SearchParams {
        query: "query-b".to_string(),
        categories: None,
        language: None,
        time_range: None,
        safesearch: None,
        page: None,
        max_results: None,
        format: OutputFormat::Json,
    };
    let result_b = search.search(&params_b).await;
    assert!(result_b.is_ok());
}
