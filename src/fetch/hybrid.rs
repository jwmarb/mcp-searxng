use crate::config::Config;
use crate::error::{CliError, Result};
use crate::fetch::{ContentFormat, FetchParams, FetchResponse, RenderMode};
use crate::retry::RetryClient;

use super::util::{extract_title, truncate_content};

const JS_HEAVY_THRESHOLD: usize = 500;

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

    let snapshot_json: serde_json::Value = snapshot_resp
        .json()
        .await
        .map_err(|e| CliError::Browser(format!("Failed to parse snapshot JSON: {e}")))?;

    let html = snapshot_json["data"]
        .as_str()
        .ok_or_else(|| CliError::Browser("Snapshot data missing".to_string()))?;

    let status_code = get_page_status(client, &session_id, browser_url).await;

    let title = super::util::extract_title(html);
    let content = html_to_markdown_rs::convert(html, None)
        .unwrap_or_default()
        .content
        .unwrap_or_default();
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
        status_code,
        content_length: content_len,
    })
}

async fn get_page_status(client: &RetryClient, session_id: &str, browser_url: &str) -> u16 {
    let resp = client
        .post_json(
            &format!("{browser_url}/api/evaluate"),
            &serde_json::json!({
                "session": session_id,
                "script": "performance.getEntriesByType('navigation')[0]?.responseStatus || 200"
            }),
        )
        .await;

    if let Ok(json) = resp {
        if let Ok(data) = json.json::<serde_json::Value>().await {
            if let Some(status) = data["data"].as_u64() {
                if (100..600).contains(&status) {
                    return status as u16;
                }
            }
        }
    }
    200
}
