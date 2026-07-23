use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

use crate::retry::RetryConfig;

/// Cache configuration for search results.
#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfig {
    #[serde(default = "default_search_ttl_secs")]
    pub search_ttl_secs: u64,

    #[serde(default = "default_max_entries")]
    pub max_entries: u64,
}

fn default_search_ttl_secs() -> u64 {
    300
}

fn default_max_entries() -> u64 {
    200
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            search_ttl_secs: default_search_ttl_secs(),
            max_entries: default_max_entries(),
        }
    }
}

/// Application configuration.
///
/// Values are loaded with this precedence (lowest to highest):
/// 1. Hard-coded defaults
/// 2. YAML config file (`~/.config/searxng-cli/config.yaml`)
/// 3. Environment variables (`SEARXNG_URL`, `SEARXNG_SERVER_PORT`, `SEARXNG_CHROME_PATH`)
/// 4. CLI flags
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_searxng_url")]
    pub searxng_url: String,

    #[serde(default = "default_server_port")]
    pub server_port: u16,

    #[serde(default)]
    pub chrome_path: Option<String>,

    #[serde(default = "default_browser_server_url")]
    pub browser_server_url: String,

    #[serde(default)]
    pub retry: RetryConfig,

    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,

    #[serde(default = "default_session_idle_timeout_secs")]
    pub session_idle_timeout_secs: u64,

    #[serde(default)]
    pub cache: CacheConfig,
}

// defaults

fn default_searxng_url() -> String {
    "http://localhost:8888".to_string()
}

fn default_server_port() -> u16 {
    18960
}

fn default_browser_server_url() -> String {
    "http://localhost:18960".to_string()
}

fn default_max_sessions() -> usize {
    8
}

fn default_session_idle_timeout_secs() -> u64 {
    600
}

/// Resolve the XDG config path for `searxng-cli/config.yaml`.
fn config_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|base| {
        base.config_dir()
            .join("searxng-cli")
            .join("config.yaml")
    })
}

// ── loading ─────────────────────────────────────────────────────────────────

impl Config {
    /// Load configuration from defaults, file, and environment variables.
    ///
    /// Returns a `Config` with values merged in precedence order:
    /// defaults → file → env vars.
    ///
    /// CLI flags are applied by the caller after parsing.
    pub fn load() -> Self {
        Self::load_with_path(None)
    }

    /// Load configuration from defaults, optional custom file, and environment variables.
    pub fn load_with_path(custom_path: Option<String>) -> Self {
        let mut config = Self::default();

        // 1. File (custom path or default XDG path)
        let path = custom_path.map(PathBuf::from).or_else(config_path);
        if let Some(path) = path {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(file_config) = serde_yaml::from_str::<Self>(&contents) {
                    config = file_config;
                }
            }
        }

        // 2. Environment variables
        if let Ok(val) = std::env::var("SEARXNG_URL") {
            config.searxng_url = val;
        }
        if let Ok(val) = std::env::var("SEARXNG_SERVER_PORT") {
            if let Ok(port) = val.parse::<u16>() {
                config.server_port = port;
            }
        }
        if let Ok(val) = std::env::var("SEARXNG_CHROME_PATH") {
            config.chrome_path = Some(val);
        }
        if let Ok(val) = std::env::var("SEARXNG_BROWSER_SERVER_URL") {
            config.browser_server_url = val;
        }

        config
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            searxng_url: default_searxng_url(),
            server_port: default_server_port(),
            chrome_path: None,
            browser_server_url: default_browser_server_url(),
            retry: RetryConfig::default(),
            max_sessions: default_max_sessions(),
            session_idle_timeout_secs: default_session_idle_timeout_secs(),
            cache: CacheConfig::default(),
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "SearXNG URL: {}", self.searxng_url)?;
        writeln!(f, "Server port: {}", self.server_port)?;
        if let Some(ref path) = self.chrome_path {
            writeln!(f, "Chrome path: {}", path)?;
        }
        writeln!(f, "Browser server URL: {}", self.browser_server_url)?;
        writeln!(f, "Max sessions: {}", self.max_sessions)?;
        writeln!(f, "Session idle timeout: {}s", self.session_idle_timeout_secs)?;
        writeln!(f, "Cache TTL: {}s, max entries: {}", self.cache.search_ttl_secs, self.cache.max_entries)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn cleanup_env() {
        std::env::remove_var("SEARXNG_URL");
        std::env::remove_var("SEARXNG_SERVER_PORT");
        std::env::remove_var("SEARXNG_CHROME_PATH");
        std::env::remove_var("SEARXNG_BROWSER_SERVER_URL");
    }

    fn with_env<F: FnOnce()>(f: F) {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("SEARXNG_URL");
        std::env::remove_var("SEARXNG_SERVER_PORT");
        std::env::remove_var("SEARXNG_CHROME_PATH");
        std::env::remove_var("SEARXNG_BROWSER_SERVER_URL");
        f();
        std::env::remove_var("SEARXNG_URL");
        std::env::remove_var("SEARXNG_SERVER_PORT");
        std::env::remove_var("SEARXNG_CHROME_PATH");
        std::env::remove_var("SEARXNG_BROWSER_SERVER_URL");
    }

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert_eq!(config.searxng_url, "http://localhost:8888");
        assert_eq!(config.server_port, 18960);
        assert_eq!(config.chrome_path, None);
        assert_eq!(config.browser_server_url, "http://localhost:18960");
        assert_eq!(config.retry.max_retries, 3);
        assert_eq!(config.retry.base_delay_ms, 200);
        assert_eq!(config.retry.timeout_secs, 15);
        assert_eq!(config.max_sessions, 8);
        assert_eq!(config.session_idle_timeout_secs, 600);
        assert_eq!(config.cache.search_ttl_secs, 300);
        assert_eq!(config.cache.max_entries, 200);
    }

    #[test]
    fn test_config_load_from_yaml_file() {
        with_env(|| {
            let mut file = tempfile::NamedTempFile::new().unwrap();
            writeln!(file, "searxng_url: http://custom:9999").unwrap();
            writeln!(file, "server_port: 3000").unwrap();
            writeln!(file, "chrome_path: /usr/bin/google-chrome").unwrap();

            let config = Config::load_with_path(Some(file.path().to_str().unwrap().to_string()));
            assert_eq!(config.searxng_url, "http://custom:9999");
            assert_eq!(config.server_port, 3000);
            assert_eq!(config.chrome_path, Some("/usr/bin/google-chrome".to_string()));
        });
    }

    #[test]
    fn test_config_load_with_path_none() {
        with_env(|| {
            let config = Config::load_with_path(None);
            assert_eq!(config.searxng_url, "http://localhost:8888");
            assert_eq!(config.server_port, 18960);
            assert_eq!(config.chrome_path, None);
            assert_eq!(config.browser_server_url, "http://localhost:18960");
        });
    }

    #[test]
    fn test_env_var_overrides_url() {
        with_env(|| {
            std::env::set_var("SEARXNG_URL", "http://env-url:7777");
            let config = Config::load_with_path(None);
            assert_eq!(config.searxng_url, "http://env-url:7777");
        });
    }

    #[test]
    fn test_env_var_overrides_port() {
        with_env(|| {
            std::env::set_var("SEARXNG_SERVER_PORT", "4000");
            let config = Config::load_with_path(None);
            assert_eq!(config.server_port, 4000);
        });
    }

    #[test]
    fn test_env_var_overrides_chrome_path() {
        with_env(|| {
            std::env::set_var("SEARXNG_CHROME_PATH", "/opt/chrome");
            let config = Config::load_with_path(None);
            assert_eq!(config.chrome_path, Some("/opt/chrome".to_string()));
        });
    }

    #[test]
    fn test_env_var_overrides_browser_server_url() {
        with_env(|| {
            std::env::set_var("SEARXNG_BROWSER_SERVER_URL", "http://custom-browser:9999");
            let config = Config::load_with_path(None);
            assert_eq!(config.browser_server_url, "http://custom-browser:9999");
        });
    }

    #[test]
    fn test_env_vars_override_file_values() {
        with_env(|| {
            let mut file = tempfile::NamedTempFile::new().unwrap();
            writeln!(file, "searxng_url: http://file:1111").unwrap();
            writeln!(file, "server_port: 2000").unwrap();

            std::env::set_var("SEARXNG_URL", "http://env:2222");
            std::env::set_var("SEARXNG_SERVER_PORT", "3000");
            std::env::set_var("SEARXNG_CHROME_PATH", "/env/chrome");
            std::env::set_var("SEARXNG_BROWSER_SERVER_URL", "http://env-browser:5555");

            let config = Config::load_with_path(Some(file.path().to_str().unwrap().to_string()));
            assert_eq!(config.searxng_url, "http://env:2222");
            assert_eq!(config.server_port, 3000);
            assert_eq!(config.chrome_path, Some("/env/chrome".to_string()));
            assert_eq!(config.browser_server_url, "http://env-browser:5555");
        });
    }

    #[test]
    fn test_display_without_chrome_path() {
        let config = Config::default();
        let output = format!("{}", config);
        assert!(output.contains("SearXNG URL: http://localhost:8888"));
        assert!(output.contains("Server port: 18960"));
        assert!(!output.contains("Chrome path:"));
        assert!(output.contains("Max sessions: 8"));
    }

    #[test]
    fn test_display_with_chrome_path() {
        let config = Config {
            searxng_url: "http://localhost:8888".to_string(),
            server_port: 18960,
            chrome_path: Some("/usr/bin/chrome".to_string()),
            browser_server_url: "http://localhost:18960".to_string(),
            retry: RetryConfig::default(),
            max_sessions: 8,
            session_idle_timeout_secs: 600,
            cache: CacheConfig::default(),
        };
        let output = format!("{}", config);
        assert!(output.contains("Chrome path: /usr/bin/chrome"));
    }

    #[test]
    fn test_nonexistent_custom_path_returns_defaults() {
        with_env(|| {
            let config = Config::load_with_path(Some("/nonexistent/path/config.yaml".to_string()));
            assert_eq!(config.searxng_url, "http://localhost:8888");
            assert_eq!(config.server_port, 18960);
            assert_eq!(config.chrome_path, None);
            assert_eq!(config.browser_server_url, "http://localhost:18960");
        });
    }
}