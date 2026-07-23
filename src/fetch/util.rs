/// Extract the `<title>` tag content from HTML.
pub fn extract_title(html: &str) -> String {
    if let Some(start) = html.find("<title>") {
        let rest = &html[start + 7..];
        if let Some(end) = rest.find("</title>") {
            return rest[..end].trim().to_string();
        }
    }
    String::new()
}

/// Truncate content to a maximum number of characters, appending "..." if truncated.
pub fn truncate_content(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        content.to_string()
    } else {
        let cutoff = content
            .char_indices()
            .take_while(|(i, _)| *i < max_chars)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(max_chars);
        format!("{}...", &content[..cutoff])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // extract_title tests

    #[test]
    fn test_extract_title_simple() {
        assert_eq!(extract_title("<title>Hello</title>"), "Hello");
    }

    #[test]
    fn test_extract_title_trims_whitespace() {
        assert_eq!(extract_title("<title>  Spaces  </title>"), "Spaces");
    }

    #[test]
    fn test_extract_title_no_tag() {
        assert_eq!(extract_title("<head>No title here</head>"), "");
    }

    #[test]
    fn test_extract_title_case_sensitive() {
        assert_eq!(extract_title("<TITLE>case</TITLE>"), "");
    }

    #[test]
    fn test_extract_title_multiline_preserved() {
        assert_eq!(extract_title("<title>multi\nline</title>"), "multi\nline");
    }

    #[test]
    fn test_extract_title_nested_in_head() {
        assert_eq!(extract_title("<head><title>Nested</title></head>"), "Nested");
    }

    // truncate_content tests (from mod.rs)

    #[test]
    fn test_truncate_short_content_unchanged() {
        let content = "short";
        assert_eq!(truncate_content(content, 100), "short");
    }

    #[test]
    fn test_truncate_exact_boundary_unchanged() {
        let content = "12345";
        assert_eq!(truncate_content(content, 5), "12345");
    }

    #[test]
    fn test_truncate_one_char_over() {
        let content = "12345";
        assert_eq!(truncate_content(content, 4), "1234...");
    }

    #[test]
    fn test_truncate_empty_string() {
        assert_eq!(truncate_content("", 10), "");
    }

    #[test]
    fn test_truncate_unicode_at_boundary() {
        let content = "é".repeat(200);
        let result = truncate_content(&content, 199 * 2);
        assert!(!result.ends_with('\u{FFFD}'));
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_zero_max_chars() {
        let content = "hello";
        let result = truncate_content(content, 0);
        assert!(result.ends_with("..."));
    }
}