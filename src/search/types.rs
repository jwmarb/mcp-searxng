use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default, deserialize_with = "deserialize_engines")]
    pub engines: Vec<String>,
    #[serde(default)]
    pub score: f64,
    #[serde(default, rename = "publishedDate")]
    pub published_date: Option<String>,
    #[serde(default, rename = "img_src")]
    pub img_src: Option<String>,
    #[serde(default, rename = "parsed_url")]
    pub parsed_url: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_option_string")]
    pub template: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_string")]
    pub thumbnail: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_string")]
    pub priority: Option<String>,
    #[serde(default)]
    pub positions: Option<Vec<i64>>,
    #[serde(default)]
    pub category: Option<String>,
}

fn deserialize_option_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrValue {
        String(String),
        Value(serde_json::Value),
    }

    let opt = Option::<StringOrValue>::deserialize(deserializer)?;
    Ok(match opt {
        Some(StringOrValue::String(s)) => Some(s),
        Some(StringOrValue::Value(v)) => v.as_str().map(|s| s.to_string()),
        None => None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    #[serde(default)]
    pub answers: Vec<serde_json::Value>,
    #[serde(default)]
    pub corrections: Vec<String>,
    #[serde(default)]
    pub suggestions: Vec<String>,
    #[serde(default)]
    pub infoboxes: Vec<serde_json::Value>,
    #[serde(default, rename = "unresponsive_engines")]
    pub unresponsive_engines: Vec<serde_json::Value>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default, rename = "number_of_results")]
    pub number_of_results: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SearchParams {
    pub query: String,
    pub categories: Option<String>,
    pub language: Option<String>,
    pub time_range: Option<String>,
    pub safesearch: Option<u8>,
    pub page: Option<u32>,
    pub max_results: Option<usize>,
    pub format: OutputFormat,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            categories: None,
            language: None,
            time_range: None,
safesearch: None,
            page: None,
            max_results: None,
            format: OutputFormat::Json,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum OutputFormat {
    #[default]
    Json,
    Text,
    Markdown,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "text" => Ok(Self::Text),
            "markdown" => Ok(Self::Markdown),
            _ => Err(format!("Unknown output format: {s}")),
        }
    }
}

fn deserialize_engines<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                match item {
                    serde_json::Value::String(s) => result.push(s),
                    serde_json::Value::Object(map) => {
                        if let Some(name) = map.get("name").and_then(|v| v.as_str()) {
                            result.push(name.to_string());
                        }
                    }
                    _ => {}
                }
            }
            Ok(result)
        }
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // SearchResponse deserialization tests

    #[test]
    fn search_response_empty_json() {
        let json = r#"{"results":[]}"#;
        let resp: SearchResponse = serde_json::from_str(json).unwrap();
        assert!(resp.results.is_empty());
        assert!(resp.answers.is_empty());
        assert!(resp.corrections.is_empty());
        assert!(resp.suggestions.is_empty());
        assert!(resp.infoboxes.is_empty());
        assert!(resp.unresponsive_engines.is_empty());
        assert!(resp.query.is_none());
        assert!(resp.number_of_results.is_none());
    }

    #[test]
    fn search_response_single_result() {
        let json = r#"{
            "results":[{"title":"Test","url":"https://example.com","content":"Hello"}]
        }"#;
        let resp: SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.results.len(), 1);
        assert_eq!(resp.results[0].title, "Test");
        assert_eq!(resp.results[0].url, "https://example.com");
    }

    #[test]
    fn search_response_multiple_results() {
        let json = r#"{
            "results":[
                {"title":"A","url":"https://a.com","content":"1"},
                {"title":"B","url":"https://b.com","content":"2"},
                {"title":"C","url":"https://c.com","content":"3"}
            ],
            "query":"test query",
            "number_of_results":42
        }"#;
        let resp: SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.results.len(), 3);
        assert_eq!(resp.query, Some("test query".into()));
        assert_eq!(resp.number_of_results, Some(42));
    }

    #[test]
    fn search_response_all_optional_fields() {
        let json = r#"{
            "results":[],
            "answers":["answer1"],
            "corrections":["corrected"],
            "suggestions":["sug1"],
            "infoboxes":[{"key":"val"}],
            "unresponsive_engines":["engine1"],
            "query":"q",
            "number_of_results":10
        }"#;
        let resp: SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.answers.len(), 1);
        assert_eq!(resp.corrections, vec!["corrected"]);
        assert_eq!(resp.suggestions, vec!["sug1"]);
        assert_eq!(resp.infoboxes.len(), 1);
        assert_eq!(resp.unresponsive_engines.len(), 1);
    }

    // SearchResult deserialization tests

    #[test]
    fn search_result_all_fields() {
        let json = r#"{
            "title":"Full","url":"https://x.com","content":"body",
            "engine":"google","engines":["google","bing"],
            "score":0.95,"publishedDate":"2024-01-01",
            "img_src":"/img.png","parsed_url":["https","x.com","/",""],
            "template":"default","thumbnail":"thumb","priority":"high",
            "positions":[1,2],"category":"general"
        }"#;
        let r: SearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.title, "Full");
        assert_eq!(r.engines, vec!["google", "bing"]);
        assert_eq!(r.published_date, Some("2024-01-01".into()));
        assert_eq!(r.img_src, Some("/img.png".into()));
        assert_eq!(r.template, Some("default".into()));
        assert_eq!(r.thumbnail, Some("thumb".into()));
        assert_eq!(r.priority, Some("high".into()));
        assert_eq!(r.positions, Some(vec![1, 2]));
        assert_eq!(r.category, Some("general".into()));
    }

    #[test]
    fn search_result_missing_optionals() {
        let json = r#"{"title":"T","url":"https://u.com","content":"C"}"#;
        let r: SearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.title, "T");
        assert_eq!(r.engine, "");
        assert!(r.engines.is_empty());
        assert_eq!(r.score, 0.0);
        assert!(r.published_date.is_none());
        assert!(r.img_src.is_none());
        assert!(r.parsed_url.is_none());
        assert!(r.template.is_none());
        assert!(r.thumbnail.is_none());
        assert!(r.priority.is_none());
        assert!(r.positions.is_none());
        assert!(r.category.is_none());
    }

    // deserialize_engines tests

    #[test]
    fn deserialize_engines_array_of_strings() {
        let json = r#"{"title":"T","url":"https://u.com","content":"C","engines":["google","duckduckgo"]}"#;
        let r: SearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.engines, vec!["google", "duckduckgo"]);
    }

    #[test]
    fn deserialize_engines_array_of_objects() {
        let json = r#"{"title":"T","url":"https://u.com","content":"C","engines":[{"name":"google"},{"name":"bing"}]}"#;
        let r: SearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.engines, vec!["google", "bing"]);
    }

    #[test]
    fn deserialize_engines_mixed() {
        let json = r#"{"title":"T","url":"https://u.com","content":"C","engines":["google",{"name":"bing"},123]}"#;
        let r: SearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.engines, vec!["google", "bing"]);
    }

    #[test]
    fn deserialize_engines_empty() {
        let json = r#"{"title":"T","url":"https://u.com","content":"C","engines":[]}"#;
        let r: SearchResult = serde_json::from_str(json).unwrap();
        assert!(r.engines.is_empty());
    }

    // deserialize_option_string tests

    #[test]
    fn deserialize_option_string_plain_string() {
        let json = r#"{"title":"T","url":"https://u.com","content":"C","template":"default"}"#;
        let r: SearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.template, Some("default".into()));
    }

    #[test]
    fn deserialize_option_string_nested_json() {
        let json = r#"{"title":"T","url":"https://u.com","content":"C","template":{"nested":"val"}}"#;
        let r: SearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.template, None);
    }

    #[test]
    fn deserialize_option_string_null() {
        let json = r#"{"title":"T","url":"https://u.com","content":"C","template":null}"#;
        let r: SearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.template, None);
    }

    // OutputFormat tests

    #[test]
    fn output_format_from_str_valid() {
        assert!(matches!(OutputFormat::from_str("json").unwrap(), OutputFormat::Json));
        assert!(matches!(OutputFormat::from_str("JSON").unwrap(), OutputFormat::Json));
        assert!(matches!(OutputFormat::from_str("text").unwrap(), OutputFormat::Text));
        assert!(matches!(OutputFormat::from_str("markdown").unwrap(), OutputFormat::Markdown));
    }

    #[test]
    fn output_format_from_str_invalid() {
        assert!(OutputFormat::from_str("invalid").is_err());
    }

    #[test]
    fn output_format_default() {
        let fmt = OutputFormat::default();
        assert!(matches!(fmt, OutputFormat::Json));
    }

    // SearchParams tests

    #[test]
    fn search_params_default() {
        let p = SearchParams::default();
        assert_eq!(p.query, "");
        assert!(p.categories.is_none());
        assert!(p.language.is_none());
        assert!(p.time_range.is_none());
        assert!(p.safesearch.is_none());
        assert!(p.page.is_none());
        assert!(p.max_results.is_none());
        assert!(matches!(p.format, OutputFormat::Json));
    }
}