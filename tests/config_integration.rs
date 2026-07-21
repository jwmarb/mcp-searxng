use searxng_cli::config::Config;
use tempfile::NamedTempFile;
use std::io::Write;

#[test]
fn test_config_load_from_yaml() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "searxng_url: \"http://custom.example.com\"").unwrap();
    writeln!(file, "server_port: 9999").unwrap();
    writeln!(file, "chrome_path: \"/custom/chrome\"").unwrap();
    
    let config = Config::load_with_path(Some(file.path().to_str().unwrap().to_string()));
    
    assert_eq!(config.searxng_url, "http://custom.example.com");
    assert_eq!(config.server_port, 9999);
    assert_eq!(config.chrome_path, Some("/custom/chrome".to_string()));
}

#[test]
fn test_config_missing_file_returns_defaults() {
    let config = Config::load_with_path(Some("/nonexistent/path/config.yaml".to_string()));
    
    assert_eq!(config.searxng_url, "http://localhost:8888");
    assert_eq!(config.server_port, 18960);
    assert_eq!(config.chrome_path, None);
}

#[test]
fn test_config_empty_yaml() {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "").unwrap();
    
    let config = Config::load_with_path(Some(file.path().to_str().unwrap().to_string()));
    
    assert_eq!(config.searxng_url, "http://localhost:8888");
    assert_eq!(config.server_port, 18960);
}
