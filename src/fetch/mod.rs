pub mod hybrid;
pub mod types;

pub use types::*;

use std::time::Duration;

use reqwest::Client;

use crate::error::Result;

pub struct Fetcher {
    client: Client,
    config: Option<crate::config::Config>,
}

impl Fetcher {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client builder"),
            config: None,
        }
    }

    pub fn with_config(mut self, config: crate::config::Config) -> Self {
        self.config = Some(config);
        self
    }

    pub async fn fetch(&self, params: &FetchParams) -> Result<FetchResponse> {
        if params.render_mode == RenderMode::Render {
            let config = self.config.as_ref().cloned().unwrap_or_default();
            return hybrid::hybrid_fetch(&config, params).await;
        }

        let timeout = params.timeout.unwrap_or(30);
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout))
            .build()?;

        let resp = client.get(&params.url).send().await?;
        let status = resp.status().as_u16();
        let body = resp.text().await?;

        let title = extract_title(&body);
        let content = html_to_markdown_rs::convert(&body, None)
            .unwrap_or_default()
            .content
            .unwrap_or_default();

        let max_chars = params.max_chars.unwrap_or(50_000);
        let content = truncate_content(&content, max_chars);

        Ok(FetchResponse {
            url: params.url.clone(),
            title,
            content,
            format: ContentFormat::Markdown,
            status_code: status,
            content_length: body.len(),
        })
    }
}

fn extract_title(html: &str) -> String {
    if let Some(start) = html.find("<title>") {
        let rest = &html[start + 7..];
        if let Some(end) = rest.find("</title>") {
            return rest[..end].trim().to_string();
        }
    }
    String::new()
}

fn truncate_content(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        content.to_string()
    } else {
        let cutoff = content
            .char_indices()
            .take_while(|(i, _)| *i < max_chars)
            .last()
            .map(|(i, _)| i + 1)
            .unwrap_or(max_chars);
        format!("{}...", &content[..cutoff])
    }
}