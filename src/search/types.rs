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