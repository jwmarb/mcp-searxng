use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResponse {
    pub url: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "format")]
    pub format: ContentFormat,
    pub status_code: u16,
    pub content_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContentFormat {
    Markdown,
    Text,
}

impl Default for ContentFormat {
    fn default() -> Self {
        Self::Markdown
    }
}

#[derive(Debug, Clone)]
pub struct FetchParams {
    pub url: String,
    pub max_chars: Option<usize>,
    pub timeout: Option<u64>,
    pub render_mode: RenderMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderMode {
    Lightweight,
    Render,
}

impl Default for RenderMode {
    fn default() -> Self {
        Self::Lightweight
    }
}