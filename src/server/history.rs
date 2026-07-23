use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;

use crate::error::Result;
use crate::time::iso_timestamp;

#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub command: String,
    pub detail: String,
    pub duration_ms: u64,
    pub success: bool,
}

pub async fn with_history<F, Fut, T>(
    history: &Arc<RwLock<HashMap<String, Vec<HistoryEntry>>>>,
    id: &str,
    command: &str,
    detail: &str,
    f: F,
) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let start = Instant::now();
    let result = f().await;
    let elapsed = start.elapsed().as_millis() as u64;
    let success = result.is_ok();
    let entry = HistoryEntry {
        timestamp: iso_timestamp(),
        command: command.to_string(),
        detail: detail.to_string(),
        duration_ms: elapsed,
        success,
    };
    let mut hist = history.write().await;
    hist.entry(id.to_string()).or_default().push(entry);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::format_timestamp;
    use std::time::Duration;

    #[tokio::test]
    async fn with_history_records_success() {
        let history = Arc::new(RwLock::new(HashMap::new()));
        let result = with_history(&history, "s1", "navigate", "https://x.com", || async {
            Ok::<&str, crate::error::CliError>("ok")
        })
        .await;

        assert_eq!(result.unwrap(), "ok");

        let hist = history.read().await;
        let entries = hist.get("s1").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "navigate");
        assert_eq!(entries[0].detail, "https://x.com");
        assert!(entries[0].success);
    }

    #[tokio::test]
    async fn with_history_records_failure() {
        let history = Arc::new(RwLock::new(HashMap::new()));
        let result = with_history(&history, "s2", "click", "#btn", || async {
            Err::<(), _>(crate::error::CliError::Browser("not found".into()))
        })
        .await;

        assert!(result.is_err());

        let hist = history.read().await;
        let entries = hist.get("s2").unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].success);
        assert_eq!(entries[0].command, "click");
    }

    #[tokio::test]
    async fn with_history_accumulates_entries() {
        let history = Arc::new(RwLock::new(HashMap::new()));

        with_history(&history, "s3", "navigate", "https://a.com", || async {
            Ok::<(), _>(())
        })
        .await
        .unwrap();

        with_history(&history, "s3", "snapshot", "", || async {
            Ok::<String, _>("snapshot data".into())
        })
        .await
        .unwrap();

        let hist = history.read().await;
        let entries = hist.get("s3").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "navigate");
        assert_eq!(entries[1].command, "snapshot");
    }

    #[tokio::test]
    async fn with_history_separate_sessions() {
        let history = Arc::new(RwLock::new(HashMap::new()));

        with_history(&history, "a", "navigate", "https://a.com", || async {
            Ok::<(), _>(())
        })
        .await
        .unwrap();

        with_history(&history, "b", "navigate", "https://b.com", || async {
            Ok::<(), _>(())
        })
        .await
        .unwrap();

        let hist = history.read().await;
        assert_eq!(hist.get("a").unwrap().len(), 1);
        assert_eq!(hist.get("b").unwrap().len(), 1);
        assert_eq!(hist.get("a").unwrap()[0].detail, "https://a.com");
        assert_eq!(hist.get("b").unwrap()[0].detail, "https://b.com");
    }
}