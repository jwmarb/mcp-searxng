pub mod types;

pub use types::*;

use std::time::Duration;

use reqwest::Client;

use crate::config::Config;
use crate::error::Result;

pub struct Search {
    client: Client,
    base_url: String,
}

impl Search {
    pub fn new(config: &Config) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client builder");

        Self {
            client,
            base_url: config.searxng_url.clone(),
        }
    }

    pub async fn search(&self, params: &SearchParams) -> Result<SearchResponse> {
        let mut url_str = self.base_url.clone();
        url_str.push_str("/search?q=");
        url_str.push_str(&url::form_urlencoded::byte_serialize(params.query.as_bytes())
            .collect::<String>());
        url_str.push_str("&format=json");

        if let Some(ref categories) = params.categories {
            url_str.push_str("&categories=");
            url_str.push_str(categories);
        }
        if let Some(ref language) = params.language {
            url_str.push_str("&language=");
            url_str.push_str(language);
        }
        if let Some(ref time_range) = params.time_range {
            url_str.push_str("&time_range=");
            url_str.push_str(time_range);
        }
        if let Some(safesearch) = params.safesearch {
            url_str.push_str("&safesearch=");
            url_str.push_str(&safesearch.to_string());
        }
        if let Some(page) = params.page {
            url_str.push_str("&pageno=");
            url_str.push_str(&page.to_string());
        }
        if let Some(max_results) = params.max_results {
            url_str.push_str("&number_of_results=");
            url_str.push_str(&max_results.to_string());
        }

        let response = self.client.get(&url_str).send().await?;

        if !response.status().is_success() {
            return Err(crate::error::CliError::Searxng(format!(
                "SearXNG returned status {}",
                response.status()
            )));
        }

        let body = response.text().await?;
        let result: SearchResponse = serde_json::from_str(&body)?;
        Ok(result)
    }

    pub fn format_response(response: &SearchResponse, format: OutputFormat) -> String {
        match format {
            OutputFormat::Json => serde_json::to_string_pretty(response).unwrap_or_default(),
            OutputFormat::Text => format_as_text(response),
            OutputFormat::Markdown => format_as_markdown(response),
        }
    }
}

fn format_as_text(response: &SearchResponse) -> String {
    let mut output = String::new();

    for (i, result) in response.results.iter().enumerate() {
        output.push_str(&format!("{}. {}\n   {}\n", i + 1, result.title, result.url));
        if !result.content.is_empty() {
            let truncated = if result.content.len() > 200 {
                format!("{}...", &result.content[..200])
            } else {
                result.content.clone()
            };
            output.push_str(&format!("   {truncated}\n"));
        }
        output.push('\n');
    }

    output
}

fn format_as_markdown(response: &SearchResponse) -> String {
    let mut output = String::new();

    for (i, result) in response.results.iter().enumerate() {
        output.push_str(&format!(
            "### {}. [{}]({})\n\n",
            i + 1,
            result.title,
            result.url
        ));
        if !result.content.is_empty() {
            let truncated = if result.content.len() > 200 {
                format!("{}...", &result.content[..200])
            } else {
                result.content.clone()
            };
            output.push_str(&format!("{}\n\n", truncated));
        }
    }

    output
}