use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;

use crate::error::{CliError, Result};
use crate::browser::{BrowserPoolHandle, TabInfo};
use crate::time::{instant_to_iso, iso_timestamp};

use super::history::{with_history, HistoryEntry};

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub created_at: String,
    pub tab_count: usize,
    pub active_tab: usize,
    pub active_url: Option<String>,
    pub history: Vec<HistoryEntry>,
}

#[derive(Clone)]
pub struct SessionManager {
    pool: BrowserPoolHandle,
    history: Arc<RwLock<HashMap<String, Vec<HistoryEntry>>>>,
}

impl SessionManager {
    pub fn new(pool: BrowserPoolHandle) -> Self {
        Self {
            pool,
            history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn snapshot(&self, id: &str) -> Result<String> {
        with_history(&self.history, id, "snapshot", "", || self.pool.snapshot(id)).await
    }

    pub async fn click(&self, id: &str, selector: &str) -> Result<()> {
        with_history(&self.history, id, "click", selector, || self.pool.click(id, selector)).await
    }

    pub async fn fill(&self, id: &str, selector: &str, value: &str) -> Result<()> {
        let detail = format!("{} = {}", selector, value);
        with_history(&self.history, id, "fill", &detail, || self.pool.fill(id, selector, value)).await
    }

    pub async fn evaluate(&self, id: &str, script: &str) -> Result<serde_json::Value> {
        with_history(&self.history, id, "evaluate", script, || self.pool.evaluate(id, script)).await
    }

    pub async fn screenshot(&self, id: &str) -> Result<Vec<u8>> {
        with_history(&self.history, id, "screenshot", "", || self.pool.screenshot(id)).await
    }

    pub async fn new_tab(&self, id: &str, url: Option<&str>) -> Result<()> {
        let detail = url.unwrap_or_default().to_string();
        with_history(&self.history, id, "new_tab", &detail, || self.pool.new_tab(id, url)).await
    }

    pub async fn close_tab(&self, id: &str, index: usize) -> Result<()> {
        with_history(&self.history, id, "close_tab", &index.to_string(), || self.pool.close_tab(id, index)).await
    }

    pub async fn select_tab(&self, id: &str, index: usize) -> Result<()> {
        with_history(&self.history, id, "select_tab", &index.to_string(), || self.pool.select_tab(id, index)).await
    }

    pub async fn navigate(&self, id: &str, url: &str) -> Result<()> {
        let start = Instant::now();
        let mut ok = true;
        if !self.pool.exists_session(id).await {
            if self.pool.new_session(id).await.is_err() {
                ok = false;
            }
        }
        if ok && self.pool.goto(id, url).await.is_err() {
            ok = false;
        }
        let result = if ok { Ok(()) } else { Err(CliError::Browser("Navigation failed".to_string())) };
        let elapsed = start.elapsed().as_millis() as u64;
        record_manual(&self.history, id, "navigate", url, elapsed, result.is_ok()).await;
        result
    }

    pub async fn kill(&self, id: &str) -> Result<()> {
        let result = self.pool.kill_session(id).await;
        if result.is_ok() {
            let mut hist = self.history.write().await;
            hist.remove(id);
        }
        result
    }

    pub async fn list_tabs(&self, id: &str) -> Result<Vec<TabInfo>> {
        self.pool.list_tabs(id).await
    }

    pub async fn pool_status(&self) -> crate::browser::pool::PoolStatus {
        self.pool.pool_status().await
    }

    pub async fn list(&self) -> Vec<SessionInfo> {
        let hist = self.history.read().await;
        self.pool.list_sessions().await
            .into_iter()
            .map(|info| {
                let session_history = hist.get(&info.id).cloned().unwrap_or_default();
                SessionInfo {
                    id: info.id,
                    created_at: instant_to_iso(info.created_at),
                    tab_count: info.tab_count,
                    active_tab: 0,
                    active_url: info.active_url,
                    history: session_history,
                }
            })
            .collect()
    }
}

async fn record_manual(
    history: &Arc<RwLock<HashMap<String, Vec<HistoryEntry>>>>,
    id: &str,
    command: &str,
    detail: &str,
    duration_ms: u64,
    success: bool,
) {
    let entry = HistoryEntry {
        timestamp: iso_timestamp(),
        command: command.to_string(),
        detail: detail.to_string(),
        duration_ms,
        success,
    };
    let mut hist = history.write().await;
    hist.entry(id.to_string()).or_default().push(entry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time;
    use std::time::Duration;

    #[test]
    fn test_format_timestamp_epoch_zero() {
        let ts = time::format_timestamp(Duration::from_secs(0));
        assert_eq!(ts, "1970-01-01T00:00:00.000000000Z");
    }

    #[test]
    fn test_format_timestamp_2023_jan_01() {
        let ts = time::format_timestamp(Duration::from_secs(1672531200));
        assert_eq!(ts, "2023-01-01T00:00:00.000000000Z");
    }

    #[test]
    fn test_format_timestamp_leap_year_2020_jan_01() {
        let ts = time::format_timestamp(Duration::from_secs(1577836800));
        assert_eq!(ts, "2020-01-01T00:00:00.000000000Z");
    }

    #[test]
    fn test_format_timestamp_year_end_boundary() {
        let ts = time::format_timestamp(Duration::from_secs(1704067199));
        assert_eq!(ts, "2023-12-31T23:59:59.000000000Z");
    }

    #[test]
    fn test_format_timestamp_with_nanos() {
        let dur = Duration::new(0, 123456789);
        let ts = time::format_timestamp(dur);
        assert_eq!(ts, "1970-01-01T00:00:00.123456789Z");
    }

    #[test]
    fn test_iso_timestamp_valid_format() {
        let ts = time::iso_timestamp();
        assert_eq!(ts.len(), 30, "iso_timestamp produced invalid length: {}", ts);
        assert!(ts.ends_with('Z'), "iso_timestamp should end with Z: {}", ts);
        let parts: Vec<&str> = ts.splitn(2, 'T').collect();
        assert_eq!(parts.len(), 2, "iso_timestamp missing T separator: {}", ts);
        let date = parts[0];
        assert_eq!(date.len(), 10, "date part wrong length: {}", date);
        assert!(date.chars().nth(4).unwrap() == '-', "date format wrong: {}", date);
        let time_part = parts[1].strip_suffix('Z').unwrap();
        assert_eq!(time_part.len(), 18, "time part wrong length: {}", time_part);
    }

    #[test]
    fn test_history_entry_serialization() {
        let entry = HistoryEntry {
            timestamp: "2023-01-01T00:00:00.000000000Z".to_string(),
            command: "navigate".to_string(),
            detail: "https://example.com".to_string(),
            duration_ms: 150,
            success: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["timestamp"], "2023-01-01T00:00:00.000000000Z");
        assert_eq!(parsed["command"], "navigate");
        assert_eq!(parsed["detail"], "https://example.com");
        assert_eq!(parsed["duration_ms"], 150);
        assert_eq!(parsed["success"], true);
    }

    #[test]
    fn test_session_info_serialization() {
        let info = SessionInfo {
            id: "sess-1".to_string(),
            created_at: "2023-06-15T12:00:00.000000000Z".to_string(),
            tab_count: 3,
            active_tab: 1,
            active_url: Some("https://example.com".to_string()),
            history: vec![HistoryEntry {
                timestamp: "2023-06-15T12:00:01.000000000Z".to_string(),
                command: "navigate".to_string(),
                detail: "https://example.com".to_string(),
                duration_ms: 100,
                success: true,
            }],
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "sess-1");
        assert_eq!(parsed["tab_count"], 3);
        assert_eq!(parsed["active_tab"], 1);
        assert_eq!(parsed["active_url"], "https://example.com");
        assert_eq!(parsed["history"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_format_timestamp_century_non_leap() {
        let ts = time::format_timestamp(Duration::from_secs(4102444800));
        assert_eq!(ts, "2100-01-01T00:00:00.000000000Z");
    }
}