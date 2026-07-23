use base64::Engine;
use reqwest::Client;
use serde::de::DeserializeOwned;
use crate::config::Config;
use crate::error::{CliError, Result};

pub struct BrowserClient {
    client: Client,
    server_url: String,
}

impl BrowserClient {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            server_url: config.browser_server_url.clone(),
        }
    }

    async fn post_ok(&self, path: &str, body: &serde_json::Value) -> Result<()> {
        let response = self.client
            .post(format!("{}{}", self.server_url, path))
            .json(body)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let response = self.client
            .get(format!("{}{}", self.server_url, path))
            .send()
            .await?;

        if response.status().is_success() {
            let body: T = response.json().await?;
            Ok(body)
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    async fn post_json<T: DeserializeOwned>(&self, path: &str, body: &serde_json::Value) -> Result<T> {
        let response = self.client
            .post(format!("{}{}", self.server_url, path))
            .json(body)
            .send()
            .await?;

        if response.status().is_success() {
            let result: T = response.json().await?;
            Ok(result)
        } else {
            Err(CliError::Browser(format!("Server error: {}", response.status())))
        }
    }

    pub async fn navigate(&self, session_id: &str, url: &str) -> Result<()> {
        self.post_ok("/api/navigate", &serde_json::json!({
            "session": session_id,
            "url": url
        })).await
    }

    pub async fn snapshot(&self, session_id: &str) -> Result<String> {
        let body: serde_json::Value = self.get_json(
            &format!("/api/snapshot?session={}", session_id)
        ).await?;
        Ok(body["data"].as_str().unwrap_or("").to_string())
    }

    pub async fn click(&self, session_id: &str, selector: &str) -> Result<()> {
        self.post_ok("/api/click", &serde_json::json!({
            "session": session_id,
            "selector": selector
        })).await
    }

    pub async fn fill(&self, session_id: &str, selector: &str, value: &str) -> Result<()> {
        self.post_ok("/api/fill", &serde_json::json!({
            "session": session_id,
            "selector": selector,
            "value": value
        })).await
    }

    pub async fn evaluate(&self, session_id: &str, script: &str) -> Result<serde_json::Value> {
        let body: serde_json::Value = self.post_json("/api/evaluate", &serde_json::json!({
            "session": session_id,
            "script": script
        })).await?;
        Ok(body["data"].clone())
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

        self.post_json("/api/tabs", &payload).await
    }

    pub async fn instances(&self) -> Result<serde_json::Value> {
        self.get_json("/api/instances").await
    }

    pub async fn kill(&self, session_id: &str) -> Result<()> {
        self.post_ok("/api/kill", &serde_json::json!({
            "session": session_id
        })).await
    }

    pub async fn session_info(&self, session_id: &str) -> Result<serde_json::Value> {
        self.get_json(&format!("/api/session/{}?info=true", session_id)).await
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
