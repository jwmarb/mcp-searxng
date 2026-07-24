pub mod types;

pub use types::*;

use crate::config::Config;
use crate::error::Result;
use crate::retry::RetryClient;

pub struct Search {
    client: RetryClient,
    base_url: String,
    cache: moka::future::Cache<u64, SearchResponse>,
}

impl Search {
    pub fn new(config: &Config) -> Self {
        let client = RetryClient::new(&config.retry);
        let cache = moka::future::Cache::builder()
            .time_to_live(std::time::Duration::from_secs(config.cache.search_ttl_secs))
            .max_capacity(config.cache.max_entries)
            .build();

        Self {
            client,
            base_url: config.searxng_url.clone(),
            cache,
        }
    }

    pub async fn ping(&self) -> bool {
        self.client.get(&self.base_url).await.is_ok()
    }

    pub async fn search(&self, params: &SearchParams) -> Result<SearchResponse> {
        let key = cache_key(params);

        if let Some(cached) = self.cache.get(&key).await {
            return Ok(cached);
        }

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

        let response = self.client.get(&url_str).await?;

        if !response.status().is_success() {
            return Err(crate::error::CliError::Searxng(format!(
                "SearXNG returned status {}",
                response.status()
            )));
        }

        let body = response.text().await?;
        let result: SearchResponse = serde_json::from_str(&body)?;

        self.cache.insert(key, result.clone()).await;
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
            output.push_str(&format!("{truncated}\n\n"));
        }
    }

    output
}

fn cache_key(params: &SearchParams) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    params.query.hash(&mut hasher);
    params.categories.hash(&mut hasher);
    params.language.hash(&mut hasher);
    params.time_range.hash(&mut hasher);
    params.safesearch.hash(&mut hasher);
    params.page.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::types::SearchResult;

    fn make_result(title: &str, url: &str, content: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            content: content.to_string(),
            engine: String::new(),
            engines: vec![],
            score: 0.0,
            published_date: None,
            img_src: None,
            parsed_url: None,
            template: None,
            thumbnail: None,
            priority: None,
            positions: None,
            category: None,
        }
    }

    #[test]
    fn test_format_response_json() {
        let response = SearchResponse {
            results: vec![make_result("Test", "https://test.com", "Content")],
            answers: vec![],
            corrections: vec![],
            suggestions: vec![],
            infoboxes: vec![],
            unresponsive_engines: vec![],
            query: None,
            number_of_results: None,
        };
        let formatted = Search::format_response(&response, OutputFormat::Json);
        assert!(formatted.contains("\"results\""));
        assert!(formatted.contains("Test"));
    }

    #[test]
    fn test_format_response_text() {
        let response = SearchResponse {
            results: vec![make_result("Test", "https://test.com", "Content")],
            answers: vec![],
            corrections: vec![],
            suggestions: vec![],
            infoboxes: vec![],
            unresponsive_engines: vec![],
            query: None,
            number_of_results: None,
        };
        let formatted = Search::format_response(&response, OutputFormat::Text);
        assert!(formatted.contains("Test"));
        assert!(formatted.contains("https://test.com"));
    }

    #[test]
    fn test_format_response_markdown() {
        let response = SearchResponse {
            results: vec![make_result("Test", "https://test.com", "Content")],
            answers: vec![],
            corrections: vec![],
            suggestions: vec![],
            infoboxes: vec![],
            unresponsive_engines: vec![],
            query: None,
            number_of_results: None,
        };
        let formatted = Search::format_response(&response, OutputFormat::Markdown);
        assert!(formatted.contains("### 1."));
        assert!(formatted.contains("[Test](https://test.com)"));
    }

    #[test]
    fn test_format_as_text_empty_results() {
        let response = SearchResponse {
            results: vec![],
            answers: vec![],
            corrections: vec![],
            suggestions: vec![],
            infoboxes: vec![],
            unresponsive_engines: vec![],
            query: None,
            number_of_results: None,
        };
        let formatted = format_as_text(&response);
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_format_as_text_long_content_truncation() {
        let long_content = "A".repeat(300);
        let response = SearchResponse {
            results: vec![make_result("Test", "https://test.com", &long_content)],
            answers: vec![],
            corrections: vec![],
            suggestions: vec![],
            infoboxes: vec![],
            unresponsive_engines: vec![],
            query: None,
            number_of_results: None,
        };
        let formatted = format_as_text(&response);
        assert!(formatted.contains("..."));
        assert!(formatted.len() < 300);
    }

    #[test]
    fn test_format_as_text_no_content() {
        let response = SearchResponse {
            results: vec![make_result("Test", "https://test.com", "")],
            answers: vec![],
            corrections: vec![],
            suggestions: vec![],
            infoboxes: vec![],
            unresponsive_engines: vec![],
            query: None,
            number_of_results: None,
        };
        let formatted = format_as_text(&response);
        assert!(formatted.contains("Test"));
        assert!(!formatted.contains("..."));
    }

    #[test]
    fn test_format_as_markdown_empty_results() {
        let response = SearchResponse {
            results: vec![],
            answers: vec![],
            corrections: vec![],
            suggestions: vec![],
            infoboxes: vec![],
            unresponsive_engines: vec![],
            query: None,
            number_of_results: None,
        };
        let formatted = format_as_markdown(&response);
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_format_as_markdown_multiple_results() {
        let response = SearchResponse {
            results: vec![
                make_result("First", "https://first.com", "Content 1"),
                make_result("Second", "https://second.com", "Content 2"),
            ],
            answers: vec![],
            corrections: vec![],
            suggestions: vec![],
            infoboxes: vec![],
            unresponsive_engines: vec![],
            query: None,
            number_of_results: None,
        };
        let formatted = format_as_markdown(&response);
        assert!(formatted.contains("### 1."));
        assert!(formatted.contains("### 2."));
        assert!(formatted.contains("[First](https://first.com)"));
        assert!(formatted.contains("[Second](https://second.com)"));
    }

    #[test]
    fn test_format_as_text_multiple_results() {
        let response = SearchResponse {
            results: vec![
                make_result("First", "https://first.com", "Content 1"),
                make_result("Second", "https://second.com", "Content 2"),
            ],
            answers: vec![],
            corrections: vec![],
            suggestions: vec![],
            infoboxes: vec![],
            unresponsive_engines: vec![],
            query: None,
            number_of_results: None,
        };
        let formatted = format_as_text(&response);
        assert!(formatted.contains("1."));
        assert!(formatted.contains("2."));
        assert!(formatted.contains("First"));
        assert!(formatted.contains("Second"));
    }

    #[test]
    fn test_format_as_markdown_long_content_truncation() {
        let long_content = "A".repeat(300);
        let response = SearchResponse {
            results: vec![make_result("Test", "https://test.com", &long_content)],
            answers: vec![],
            corrections: vec![],
            suggestions: vec![],
            infoboxes: vec![],
            unresponsive_engines: vec![],
            query: None,
            number_of_results: None,
        };
        let formatted = format_as_markdown(&response);
        assert!(formatted.contains("..."));
        assert!(formatted.len() < 300);
    }
}