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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentFormat {
    #[default]
    Markdown,
    Text,
}

#[derive(Debug, Clone)]
pub struct FetchParams {
    pub url: String,
    pub max_chars: Option<usize>,
    pub timeout: Option<u64>,
    pub render_mode: RenderMode,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RenderMode {
    #[default]
    Lightweight,
    Render,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_format_default_is_markdown() {
        assert_eq!(ContentFormat::default(), ContentFormat::Markdown);
    }

    #[test]
    fn test_render_mode_default_is_lightweight() {
        assert_eq!(RenderMode::default(), RenderMode::Lightweight);
    }

    #[test]
    fn test_content_format_equality() {
        assert_eq!(ContentFormat::Markdown, ContentFormat::Markdown);
    }

    #[test]
    fn test_content_format_inequality() {
        assert_ne!(ContentFormat::Markdown, ContentFormat::Text);
    }

    #[test]
    fn test_render_mode_equality() {
        assert_eq!(RenderMode::Lightweight, RenderMode::Lightweight);
    }

    #[test]
    fn test_render_mode_inequality() {
        assert_ne!(RenderMode::Lightweight, RenderMode::Render);
    }
}