use crate::config::Config;
use crate::error::{CliError, Result};
use crate::fetch::{ContentFormat, FetchParams, FetchResponse, RenderMode};
use crate::retry::RetryClient;

use super::util::{extract_title, truncate_content};

const JS_HEAVY_THRESHOLD: usize = 100;

pub async fn hybrid_fetch(
    config: &Config,
    client: &RetryClient,
    params: &FetchParams,
) -> Result<FetchResponse> {
    if params.render_mode == RenderMode::Render {
        return browser_fetch(config, client, params).await;
    }

    let resp = client
        .get(&params.url)
        .await
        .map_err(CliError::from)?;

    let status = resp.status().as_u16();
    let body = resp.text().await
        .map_err(CliError::from)?;

    let title = extract_title(&body);
    let content = html_to_markdown_rs::convert(&body, None)
        .unwrap_or_default()
        .content
        .unwrap_or_default();

    if content.len() >= JS_HEAVY_THRESHOLD {
        let max_chars = params.max_chars.unwrap_or(50_000);
        let content = truncate_content(&content, max_chars);
        let content_len = content.len();

        return Ok(FetchResponse {
            url: params.url.clone(),
            title,
            content,
            format: ContentFormat::Markdown,
            status_code: status,
            content_length: content_len,
        });
    }

    browser_fetch(config, client, params).await
}

async fn browser_fetch(
    config: &Config,
    client: &RetryClient,
    params: &FetchParams,
) -> Result<FetchResponse> {
    let browser_url = &config.browser_server_url;

    let session_id = format!("sess-{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis()));

    let navigate_resp = client
        .post_json(
            &format!("{browser_url}/api/navigate"),
            &serde_json::json!({"session": &session_id, "url": &params.url}),
        )
        .await
        .map_err(|e| CliError::Browser(format!("Browser navigation failed: {e}")))?;

    if !navigate_resp.status().is_success() {
        return Err(CliError::Browser(format!(
            "Browser navigation returned status {}",
            navigate_resp.status()
        )));
    }

    let snapshot_resp = client
        .get(&format!("{browser_url}/api/snapshot?session={session_id}"))
        .await
        .map_err(|e| CliError::Browser(format!("Browser snapshot failed: {e}")))?;

    let snapshot_text = snapshot_resp
        .text()
        .await
        .map_err(|e| CliError::Browser(format!("Failed to read snapshot: {e}")))?;

    let title = extract_title_from_snapshot(&snapshot_text);
    let content = clean_snapshot_content(&snapshot_text);
    let max_chars = params.max_chars.unwrap_or(50_000);
    let content = truncate_content(&content, max_chars);
    let content_len = content.len();

    // Kill the temporary session to avoid leaking resources
    let _ = client
        .post_json(
            &format!("{browser_url}/api/kill"),
            &serde_json::json!({"session": &session_id}),
        )
        .await;

    Ok(FetchResponse {
        url: params.url.clone(),
        title,
        content,
        format: ContentFormat::Markdown,
        status_code: 200,
        content_length: content_len,
    })
}

fn extract_title_from_snapshot(snapshot: &str) -> String {
    for line in snapshot.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return trimmed.strip_prefix("# ").unwrap_or(trimmed).to_string();
        }
    }
    String::new()
}

fn clean_snapshot_content(snapshot: &str) -> String {
    snapshot
        .lines()
        .filter(|line| !line.trim().starts_with("[box="))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title_from_snapshot_h1() {
        let snapshot = "# Page Title\ncontent here";
        assert_eq!(extract_title_from_snapshot(snapshot), "Page Title");
    }

    #[test]
    fn test_extract_title_from_snapshot_h2_ignored() {
        let snapshot = "## Not h1\ncontent";
        assert_eq!(extract_title_from_snapshot(snapshot), "");
    }

    #[test]
    fn test_extract_title_from_snapshot_no_heading() {
        let snapshot = "No heading here";
        assert_eq!(extract_title_from_snapshot(snapshot), "");
    }

    #[test]
    fn test_extract_title_from_snapshot_indented() {
        let snapshot = "  # Indented Title";
        assert_eq!(extract_title_from_snapshot(snapshot), "Indented Title");
    }

    #[test]
    fn test_extract_title_from_snapshot_multiple() {
        let snapshot = "# First\n# Second";
        assert_eq!(extract_title_from_snapshot(snapshot), "First");
    }

    #[test]
    fn test_clean_snapshot_content_removes_box_lines() {
        let snapshot = "[box=1,2,3,4]\nNormal Line\n[box=5,6,7,8]";
        assert_eq!(clean_snapshot_content(snapshot), "Normal Line");
    }

    #[test]
    fn test_clean_snapshot_content_keeps_normal_lines() {
        let snapshot = "Line 1\nLine 2\nLine 3";
        assert_eq!(clean_snapshot_content(snapshot), "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_clean_snapshot_content_empty() {
        assert_eq!(clean_snapshot_content(""), "");
    }

    #[test]
    fn test_clean_snapshot_content_preserves_whitespace() {
        let snapshot = "  Indented\n[box=1,2,3,4]";
        assert_eq!(clean_snapshot_content(snapshot), "  Indented");
    }
}
