use base64::Engine;
use reqwest::Client;
use crate::config::Config;
use crate::error::{CliError, Result};

pub struct BrowserClient {
    config: Config,
    client: Client,
    server_url: String,
}

impl BrowserClient {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
            client: Client::new(),
            server_url: config.browser_server_url.clone(),
        }
    }

    pub async fn navigate(&self, session_id: &str, url: &str) -> Result<()> {
        let response = self.client
            .post(format!("{}/api/navigate", self.server_url))
            .json(&serde_json::json!({
                "session": session_id,
                "url": url
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    pub async fn snapshot(&self, session_id: &str) -> Result<String> {
        let response = self.client
            .get(format!("{}/api/snapshot?session={}", self.server_url, session_id))
            .send()
            .await?;

        if response.status().is_success() {
            let body: serde_json::Value = response.json().await?;
            Ok(body["content"].as_str().unwrap_or("").to_string())
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    pub async fn click(&self, session_id: &str, selector: &str) -> Result<()> {
        let response = self.client
            .post(format!("{}/api/click", self.server_url))
            .json(&serde_json::json!({
                "session": session_id,
                "selector": selector
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    pub async fn fill(&self, session_id: &str, selector: &str, value: &str) -> Result<()> {
        let response = self.client
            .post(format!("{}/api/fill", self.server_url))
            .json(&serde_json::json!({
                "session": session_id,
                "selector": selector,
                "value": value
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    pub async fn evaluate(&self, session_id: &str, script: &str) -> Result<serde_json::Value> {
        let response = self.client
            .post(format!("{}/api/evaluate", self.server_url))
            .json(&serde_json::json!({
                "session": session_id,
                "script": script
            }))
            .send()
            .await?;

        if response.status().is_success() {
            let body: serde_json::Value = response.json().await?;
            Ok(body["result"].clone())
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    pub async fn screenshot(&self, session_id: &str, file_path: Option<&str>) -> Result<()> {
        let response = self.client
            .get(format!("{}/api/screenshot?session={}", self.server_url, session_id))
            .send()
            .await?;

        if response.status().is_success() {
            let bytes = response.bytes().await?;
            if let Some(path) = file_path {
                std::fs::write(path, &bytes)?;
                println!("Screenshot saved to {}", path);
            } else {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                println!("{}", encoded);
            }
            Ok(())
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    pub async fn tabs(&self, session_id: &str, action: Option<&str>, url: Option<&str>) -> Result<serde_json::Value> {
        let mut payload = serde_json::json!({
            "session": session_id,
        });

        if let Some(a) = action {
            payload["action"] = serde_json::json!(a);
        }
        if let Some(u) = url {
            payload["url"] = serde_json::json!(u);
        }

        let response = self.client
            .post(format!("{}/api/tabs", self.server_url))
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let body: serde_json::Value = response.json().await?;
            Ok(body)
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    pub async fn instances(&self) -> Result<serde_json::Value> {
        let response = self.client
            .get(format!("{}/api/instances", self.server_url))
            .send()
            .await?;

        if response.status().is_success() {
            let body: serde_json::Value = response.json().await?;
            Ok(body)
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    pub async fn kill(&self, session_id: &str) -> Result<()> {
        let response = self.client
            .post(format!("{}/api/kill", self.server_url))
            .json(&serde_json::json!({
                "session": session_id
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    pub async fn session_info(&self, session_id: &str) -> Result<serde_json::Value> {
        let response = self.client
            .get(format!("{}/api/session/{}?info=true", self.server_url, session_id))
            .send()
            .await?;

        if response.status().is_success() {
            let body: serde_json::Value = response.json().await?;
            Ok(body)
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_client_new() {
        let config = Config::default();
        let client = BrowserClient::new(&config);
        assert_eq!(client.server_url, "http://localhost:18960");
    }

    #[test]
    fn test_browser_client_custom_port() {
        let config = Config {
            browser_server_url: "http://localhost:9999".to_string(),
            ..Config::default()
        };
        let client = BrowserClient::new(&config);
        assert_eq!(client.server_url, "http://localhost:9999");
    }
}