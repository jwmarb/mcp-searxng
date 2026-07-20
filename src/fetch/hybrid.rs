use crate::config::Config;
use crate::error::{CliError, Result};
use crate::fetch::{ContentFormat, FetchParams, FetchResponse, RenderMode};

const BROWSER_SERVER_URL: &str = "http://localhost:18960";
const JS_HEAVY_THRESHOLD: usize = 100;

pub async fn hybrid_fetch(_config: &Config, params: &FetchParams) -> Result<FetchResponse> {
    if params.render_mode == RenderMode::Render {
        return browser_fetch(params).await;
    }

    let client = reqwest::Client::new();
    let resp = client.get(&params.url).send().await
        .map_err(|e| CliError::Http(e))?;
    
    let status = resp.status().as_u16();
    let body = resp.text().await
        .map_err(|e| CliError::Http(e))?;

    let title = extract_title(&body);
    let content = html_to_markdown_rs::convert(&body, None)
        .unwrap_or_default()
        .content
        .unwrap_or_default();

    if content.len() >= JS_HEAVY_THRESHOLD {
        let max_chars = params.max_chars.unwrap_or(50_000);
        let content = truncate_content(&content, max_chars);

        return Ok(FetchResponse {
            url: params.url.clone(),
            title,
            content,
            format: ContentFormat::Markdown,
            status_code: status,
            content_length: body.len(),
        });
    }

    browser_fetch(params).await
}

async fn browser_fetch(params: &FetchParams) -> Result<FetchResponse> {
    let client = reqwest::Client::new();

    let session_id = format!("sess-{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis()));
    
    let navigate_resp = client
        .post(format!("{BROWSER_SERVER_URL}/api/navigate"))
        .json(&serde_json::json!({"session": &session_id, "url": &params.url}))
        .send()
        .await
        .map_err(|e| CliError::Browser(format!("Browser navigation failed: {e}")))?;

    if !navigate_resp.status().is_success() {
        return Err(CliError::Browser(format!(
            "Browser navigation returned status {}",
            navigate_resp.status()
        )));
    }

    let snapshot_resp = client
        .get(format!("{BROWSER_SERVER_URL}/api/snapshot?session={session_id}"))
        .send()
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

    Ok(FetchResponse {
        url: params.url.clone(),
        title,
        content,
        format: ContentFormat::Markdown,
        status_code: 200,
        content_length: snapshot_text.len(),
    })
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