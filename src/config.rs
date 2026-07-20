use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

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
}

// defaults

fn default_searxng_url() -> String {
    "http://localhost:8888".to_string()
}

fn default_server_port() -> u16 {
    18960
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

        config
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            searxng_url: default_searxng_url(),
            server_port: default_server_port(),
            chrome_path: None,
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
        Ok(())
    }
}