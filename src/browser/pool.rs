use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;

use super::BrowserManager;
use crate::error::{CliError, Result};

const MAX_HISTORY: usize = 50;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TabInfo {
    pub index: usize,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub created_at: Instant,
    pub tab_count: usize,
    pub active_url: Option<String>,
    pub history: Vec<String>,
}

struct SessionData {
    context: playwright_cdp::BrowserContext,
    created_at: Instant,
    active_page_index: usize,
    history: Vec<String>,
}

struct PoolInner {
    browser: Arc<BrowserManager>,
    sessions: HashMap<String, SessionData>,
}

enum PoolCmd {
    NewSession {
        id: String,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },
    KillSession {
        id: String,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },
    ListSessions {
        reply: tokio::sync::oneshot::Sender<Vec<SessionInfo>>,
    },
    Goto {
        id: String,
        url: String,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },
    Snapshot {
        id: String,
        reply: tokio::sync::oneshot::Sender<Result<String>>,
    },
    Evaluate {
        id: String,
        script: String,
        reply: tokio::sync::oneshot::Sender<Result<serde_json::Value>>,
    },
    Screenshot {
        id: String,
        reply: tokio::sync::oneshot::Sender<Result<Vec<u8>>>,
    },
    Click {
        id: String,
        selector: String,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },
    Fill {
        id: String,
        selector: String,
        value: String,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },
    NewTab {
        id: String,
        url: Option<String>,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },
    CloseTab {
        id: String,
        index: usize,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },
    SelectTab {
        id: String,
        index: usize,
        reply: tokio::sync::oneshot::Sender<Result<()>>,
    },
    ListTabs {
        id: String,
        reply: tokio::sync::oneshot::Sender<Result<Vec<TabInfo>>>,
    },
    SessionCount {
        reply: tokio::sync::oneshot::Sender<usize>,
    },
    ExistsSession {
        id: String,
        reply: tokio::sync::oneshot::Sender<bool>,
    },
}

#[derive(Clone)]
pub struct BrowserPoolHandle {
    tx: mpsc::UnboundedSender<PoolCmd>,
}

impl BrowserPoolHandle {
    pub fn new(browser: Arc<BrowserManager>) -> Self {
        let inner = PoolInner {
            browser,
            sessions: HashMap::new(),
        };
        let (tx, rx) = mpsc::unbounded_channel();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create browser runtime");

        std::thread::spawn(move || {
            rt.block_on(async move {
                let mut pool = inner;
                let mut rx = rx;
                while let Some(cmd) = rx.recv().await {
                    match cmd {
                        PoolCmd::NewSession { id, reply } => {
                            let result = match pool.browser.get_browser().await {
                                Ok(browser) => {
                                    match browser.new_context().await {
                                        Ok(context) => {
                                            let _ = context.new_page().await;
                                            pool.sessions.insert(id, SessionData {
                                                context,
                                                created_at: Instant::now(),
                                                active_page_index: 0,
                                                history: Vec::new(),
                                            });
                                            Ok(())
                                        }
                                        Err(e) => Err(CliError::Browser(format!("Failed to create context: {e}"))),
                                    }
                                }
                                Err(e) => Err(e),
                            };
                            let _ = reply.send(result);
                        }
                        PoolCmd::KillSession { id, reply } => {
                            let result = match pool.sessions.remove(&id) {
                                Some(session) => {
                                    let _ = session.context.close().await;
                                    Ok(())
                                }
                                None => Err(CliError::SessionNotFound(id)),
                            };
                            let _ = reply.send(result);
                        }
                        PoolCmd::ListSessions { reply } => {
                            let sessions: Vec<SessionInfo> = pool.sessions.iter()
                                .map(|(sid, data)| {
                                    let pages = data.context.pages();
                                    SessionInfo {
                                        id: sid.clone(),
                                        created_at: data.created_at,
                                        tab_count: pages.len(),
                                        active_url: None,
                                        history: data.history.clone(),
                                    }
                                })
                                .collect();
                            let _ = reply.send(sessions);
                        }
                        PoolCmd::Goto { id, url, reply } => {
                            let result = handle_goto(&mut pool, &id, &url).await;
                            let _ = reply.send(result);
                        }
                        PoolCmd::Snapshot { id, reply } => {
                            let result = handle_snapshot(&pool, &id).await;
                            let _ = reply.send(result);
                        }
                        PoolCmd::Evaluate { id, script, reply } => {
                            let result = handle_evaluate(&pool, &id, &script).await;
                            let _ = reply.send(result);
                        }
                        PoolCmd::Screenshot { id, reply } => {
                            let result = handle_screenshot(&pool, &id).await;
                            let _ = reply.send(result);
                        }
                        PoolCmd::Click { id, selector, reply } => {
                            let result = handle_click(&pool, &id, &selector).await;
                            let _ = reply.send(result);
                        }
                        PoolCmd::Fill { id, selector, value, reply } => {
                            let result = handle_fill(&pool, &id, &selector, &value).await;
                            let _ = reply.send(result);
                        }
                        PoolCmd::NewTab { id, url, reply } => {
                            let result = handle_new_tab(&mut pool, &id, url.as_deref()).await;
                            let _ = reply.send(result);
                        }
                        PoolCmd::CloseTab { id, index, reply } => {
                            let result = handle_close_tab(&mut pool, &id, index).await;
                            let _ = reply.send(result);
                        }
                        PoolCmd::SelectTab { id, index, reply } => {
                            let result = handle_select_tab(&mut pool, &id, index).await;
                            let _ = reply.send(result);
                        }
                        PoolCmd::ListTabs { id, reply } => {
                            let result = handle_list_tabs(&pool, &id).await;
                            let _ = reply.send(result);
                        }
                        PoolCmd::SessionCount { reply } => {
                            let _ = reply.send(pool.sessions.len());
                        }
                        PoolCmd::ExistsSession { id, reply } => {
                            let _ = reply.send(pool.sessions.contains_key(&id));
                        }
                    }
                }
            });
        });

        Self { tx }
    }

    pub async fn new_session(&self, id: &str) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::NewSession { id: id.to_string(), reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn kill_session(&self, id: &str) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::KillSession { id: id.to_string(), reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn list_sessions(&self) -> Vec<SessionInfo> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::ListSessions { reply: tx }).ok();
        rx.await.unwrap_or_default()
    }

    pub async fn goto(&self, id: &str, url: &str) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::Goto { id: id.to_string(), url: url.to_string(), reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn snapshot(&self, id: &str) -> Result<String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::Snapshot { id: id.to_string(), reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn evaluate(&self, id: &str, script: &str) -> Result<serde_json::Value> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::Evaluate { id: id.to_string(), script: script.to_string(), reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn screenshot(&self, id: &str) -> Result<Vec<u8>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::Screenshot { id: id.to_string(), reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn click(&self, id: &str, selector: &str) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::Click { id: id.to_string(), selector: selector.to_string(), reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn fill(&self, id: &str, selector: &str, value: &str) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::Fill { id: id.to_string(), selector: selector.to_string(), value: value.to_string(), reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn new_tab(&self, id: &str, url: Option<&str>) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::NewTab { id: id.to_string(), url: url.map(String::from), reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn close_tab(&self, id: &str, index: usize) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::CloseTab { id: id.to_string(), index, reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn select_tab(&self, id: &str, index: usize) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::SelectTab { id: id.to_string(), index, reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn list_tabs(&self, id: &str) -> Result<Vec<TabInfo>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::ListTabs { id: id.to_string(), reply: tx })
            .map_err(|_| CliError::Browser("Pool disconnected".to_string()))?;
        rx.await.map_err(|_| CliError::Browser("Pool dropped".to_string()))?
    }

    pub async fn session_count(&self) -> usize {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::SessionCount { reply: tx }).ok();
        rx.await.unwrap_or_default()
    }

    pub async fn exists_session(&self, id: &str) -> bool {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(PoolCmd::ExistsSession { id: id.to_string(), reply: tx }).ok();
        rx.await.unwrap_or(false)
    }
}

fn get_session<'a>(pool: &'a PoolInner, id: &str) -> Result<&'a SessionData> {
    pool.sessions.get(id).ok_or_else(|| CliError::SessionNotFound(id.to_string()))
}

fn get_active_page(pool: &PoolInner, id: &str) -> Result<playwright_cdp::Page> {
    let session = get_session(pool, id)?;
    let pages = session.context.pages();
    let index = session.active_page_index.min(pages.len().saturating_sub(1));
    pages.get(index).cloned()
        .ok_or_else(|| CliError::Browser("No active page".to_string()))
}

async fn handle_goto(pool: &mut PoolInner, id: &str, url: &str) -> Result<()> {
    let page = get_active_page(pool, id)?;
    page.goto(url, None).await
        .map_err(|e| CliError::Browser(format!("Navigation failed: {e}")))?;
    if let Some(session) = pool.sessions.get_mut(id) {
        session.history.push(url.to_string());
        if session.history.len() > MAX_HISTORY {
            session.history.remove(0);
        }
    }
    Ok(())
}

async fn handle_snapshot(pool: &PoolInner, id: &str) -> Result<String> {
    let page = get_active_page(pool, id)?;
    page.content().await
        .map_err(|e| CliError::Browser(format!("Snapshot failed: {e}")))
}

async fn handle_evaluate(pool: &PoolInner, id: &str, script: &str) -> Result<serde_json::Value> {
    let page = get_active_page(pool, id)?;
    page.evaluate(script).await
        .map_err(|e| CliError::Browser(format!("Evaluation failed: {e}")))
}

async fn handle_screenshot(pool: &PoolInner, id: &str) -> Result<Vec<u8>> {
    let page = get_active_page(pool, id)?;
    page.screenshot(None).await
        .map_err(|e| CliError::Browser(format!("Screenshot failed: {e}")))
}

async fn handle_click(pool: &PoolInner, id: &str, selector: &str) -> Result<()> {
    let page = get_active_page(pool, id)?;
    page.locator(selector).click(None).await
        .map_err(|e| CliError::Browser(format!("Click failed: {e}")))
}

async fn handle_fill(pool: &PoolInner, id: &str, selector: &str, value: &str) -> Result<()> {
    let page = get_active_page(pool, id)?;
    page.locator(selector).fill(value, None).await
        .map_err(|e| CliError::Browser(format!("Fill failed: {e}")))
}

async fn handle_new_tab(pool: &mut PoolInner, id: &str, url: Option<&str>) -> Result<()> {
    let session = get_session(pool, id)?;
    let page = session.context.new_page().await
        .map_err(|e| CliError::Browser(format!("Failed to create page: {e}")))?;
    if let Some(target_url) = url {
        page.goto(target_url, None).await
            .map_err(|e| CliError::Browser(format!("Navigation failed: {e}")))?;
    }
    if let Some(session) = pool.sessions.get_mut(id) {
        let pages = session.context.pages();
        session.active_page_index = pages.len() - 1;
    }
    Ok(())
}

async fn handle_close_tab(pool: &mut PoolInner, id: &str, index: usize) -> Result<()> {
    let session = get_session(pool, id)?;
    let pages = session.context.pages();
    if pages.len() <= 1 {
        return Err(CliError::Browser("Cannot close the last tab".to_string()));
    }
    if index == 0 && pages.len() == 2 {
        return Err(CliError::Browser("Cannot close the first tab when it is the last remaining tab".to_string()));
    }
    if index >= pages.len() {
        return Err(CliError::Browser(format!("Tab index {index} out of bounds ({} tabs)", pages.len())));
    }
    if let Some(page) = pages.get(index) {
        let _ = page.close().await;
    }
    let new_len = pages.len().saturating_sub(1);
    if let Some(session) = pool.sessions.get_mut(id) {
        if session.active_page_index >= new_len {
            session.active_page_index = new_len.saturating_sub(1);
        }
    }
    Ok(())
}

async fn handle_select_tab(pool: &mut PoolInner, id: &str, index: usize) -> Result<()> {
    let session = get_session(pool, id)?;
    let pages = session.context.pages();
    if index >= pages.len() {
        return Err(CliError::Browser(format!("Tab index {index} out of bounds ({} tabs)", pages.len())));
    }
    if let Some(session) = pool.sessions.get_mut(id) {
        session.active_page_index = index;
    }
    Ok(())
}

async fn handle_list_tabs(pool: &PoolInner, id: &str) -> Result<Vec<TabInfo>> {
    let session = get_session(pool, id)?;
    let pages = session.context.pages();
    let mut tabs = Vec::new();
    for (i, page) in pages.iter().enumerate() {
        let title = page.title().await.unwrap_or_default();
        let url = page.url().await.unwrap_or_default();
        tabs.push(TabInfo { index: i, title, url });
    }
    Ok(tabs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_info_creation() {
        let info = SessionInfo {
            id: "sess-1".to_string(),
            created_at: Instant::now(),
            tab_count: 3,
            active_url: Some("https://test.com".to_string()),
            history: vec!["https://first.com".to_string(), "https://second.com".to_string()],
        };
        assert_eq!(info.id, "sess-1");
        assert_eq!(info.tab_count, 3);
        assert_eq!(info.active_url, Some("https://test.com".to_string()));
        assert_eq!(info.history.len(), 2);
    }

    #[test]
    fn test_session_info_no_active_url() {
        let info = SessionInfo {
            id: "sess-1".to_string(),
            created_at: Instant::now(),
            tab_count: 0,
            active_url: None,
            history: vec![],
        };
        assert_eq!(info.id, "sess-1");
        assert_eq!(info.tab_count, 0);
        assert_eq!(info.active_url, None);
        assert_eq!(info.history, Vec::<String>::new());
    }

    #[test]
    fn test_tab_info_creation() {
        let tab = TabInfo {
            index: 0,
            title: "Test".to_string(),
            url: "https://test.com".to_string(),
        };
        assert_eq!(tab.index, 0);
        assert_eq!(tab.title, "Test");
        assert_eq!(tab.url, "https://test.com");
    }

    #[test]
    fn test_tab_info_clone() {
        let tab = TabInfo {
            index: 1,
            title: "Page".to_string(),
            url: "https://page.com".to_string(),
        };
        let cloned = tab.clone();
        assert_eq!(cloned.index, tab.index);
        assert_eq!(cloned.title, tab.title);
        assert_eq!(cloned.url, tab.url);
    }

    #[test]
    fn test_session_info_clone() {
        let info = SessionInfo {
            id: "sess-1".to_string(),
            created_at: Instant::now(),
            tab_count: 2,
            active_url: Some("https://test.com".to_string()),
            history: vec!["https://first.com".to_string()],
        };
        let cloned = info.clone();
        assert_eq!(cloned.id, info.id);
        assert_eq!(cloned.tab_count, info.tab_count);
        assert_eq!(cloned.active_url, info.active_url);
        assert_eq!(cloned.history, info.history);
    }
}
