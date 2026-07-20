use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::RwLock;

use crate::error::{CliError, Result};
use crate::browser::{BrowserPoolHandle, TabInfo};

#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub command: String,
    pub detail: String,
    pub duration_ms: u64,
    pub success: bool,
}

fn iso_timestamp() -> String {
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format_timestamp(dur)
}

fn instant_to_iso(instant: Instant) -> String {
    let now = SystemTime::now();
    let elapsed = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let instant_elapsed = instant.elapsed();
    let created = elapsed.saturating_sub(instant_elapsed);
    format_timestamp(created)
}

fn format_timestamp(dur: std::time::Duration) -> String {
    let secs = dur.as_secs();
    let nanos = dur.subsec_nanos();

    // UTC → broken-down date/time (no external crate)
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    let ss = tod % 60;

    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let yd = if leap { 366 } else { 365 };
        if d < yd { break; }
        d -= yd;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    let md = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 1u32;
    for &mlen in &md {
        if d < mlen as i64 { break; }
        d -= mlen as i64;
        m += 1;
    }
    let day = d + 1;

    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z", y, m, day, hh, mm, ss, nanos)
}

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

    async fn record(&self, id: &str, command: &str, detail: &str, duration_ms: u64, success: bool) {
        let entry = HistoryEntry {
            timestamp: iso_timestamp(),
            command: command.to_string(),
            detail: detail.to_string(),
            duration_ms,
            success,
        };
        let mut hist = self.history.write().await;
        hist.entry(id.to_string()).or_default().push(entry);
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
        let elapsed = start.elapsed().as_millis() as u64;
        self.record(id, "navigate", url, elapsed, ok).await;
        if ok { Ok(()) } else { Err(CliError::Browser("Navigation failed".to_string())) }
    }

    pub async fn snapshot(&self, id: &str) -> Result<String> {
        let start = Instant::now();
        let result = self.pool.snapshot(id).await;
        let elapsed = start.elapsed().as_millis() as u64;
        let success = result.is_ok();
        self.record(id, "snapshot", "", elapsed, success).await;
        result
    }

    pub async fn click(&self, id: &str, selector: &str) -> Result<()> {
        let start = Instant::now();
        let result = self.pool.click(id, selector).await;
        let elapsed = start.elapsed().as_millis() as u64;
        let success = result.is_ok();
        self.record(id, "click", selector, elapsed, success).await;
        result
    }

    pub async fn fill(&self, id: &str, selector: &str, value: &str) -> Result<()> {
        let start = Instant::now();
        let result = self.pool.fill(id, selector, value).await;
        let elapsed = start.elapsed().as_millis() as u64;
        let success = result.is_ok();
        let detail = format!("{} = {}", selector, value);
        self.record(id, "fill", &detail, elapsed, success).await;
        result
    }

    pub async fn evaluate(&self, id: &str, script: &str) -> Result<serde_json::Value> {
        let start = Instant::now();
        let result = self.pool.evaluate(id, script).await;
        let elapsed = start.elapsed().as_millis() as u64;
        let success = result.is_ok();
        self.record(id, "evaluate", script, elapsed, success).await;
        result
    }

    pub async fn screenshot(&self, id: &str) -> Result<Vec<u8>> {
        let start = Instant::now();
        let result = self.pool.screenshot(id).await;
        let elapsed = start.elapsed().as_millis() as u64;
        let success = result.is_ok();
        self.record(id, "screenshot", "", elapsed, success).await;
        result
    }

    pub async fn new_tab(&self, id: &str, url: Option<&str>) -> Result<()> {
        let start = Instant::now();
        let result = self.pool.new_tab(id, url).await;
        let elapsed = start.elapsed().as_millis() as u64;
        let success = result.is_ok();
        let detail = url.map(|u| u.to_string()).unwrap_or_default();
        self.record(id, "new_tab", &detail, elapsed, success).await;
        result
    }

    pub async fn close_tab(&self, id: &str, index: usize) -> Result<()> {
        let start = Instant::now();
        let result = self.pool.close_tab(id, index).await;
        let elapsed = start.elapsed().as_millis() as u64;
        let success = result.is_ok();
        self.record(id, "close_tab", &index.to_string(), elapsed, success).await;
        result
    }

    pub async fn select_tab(&self, id: &str, index: usize) -> Result<()> {
        let start = Instant::now();
        let result = self.pool.select_tab(id, index).await;
        let elapsed = start.elapsed().as_millis() as u64;
        let success = result.is_ok();
        self.record(id, "select_tab", &index.to_string(), elapsed, success).await;
        result
    }

    pub async fn list_tabs(&self, id: &str) -> Result<Vec<TabInfo>> {
        self.pool.list_tabs(id).await
    }

    pub async fn kill(&self, id: &str) -> Result<()> {
        let result = self.pool.kill_session(id).await;
        if result.is_ok() {
            let mut hist = self.history.write().await;
            hist.remove(id);
        }
        result
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
