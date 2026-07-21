use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::RwLock;

use crate::error::Result;

#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub command: String,
    pub detail: String,
    pub duration_ms: u64,
    pub success: bool,
}

pub fn iso_timestamp() -> String {
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format_timestamp(dur)
}

pub fn instant_to_iso(instant: Instant) -> String {
    let now = SystemTime::now();
    let elapsed = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let instant_elapsed = instant.elapsed();
    let created = elapsed.saturating_sub(instant_elapsed);
    format_timestamp(created)
}

pub fn format_timestamp(dur: Duration) -> String {
    let secs = dur.as_secs();
    let nanos = dur.subsec_nanos();

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

/// Execute an async operation and automatically record it to session history.
///
/// The timing, success/failure, and entry construction are handled here so
/// callers only need to provide the operation and its metadata.
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

    #[test]
    fn test_format_timestamp_epoch_zero() {
        let ts = format_timestamp(Duration::from_secs(0));
        assert_eq!(ts, "1970-01-01T00:00:00.000000000Z");
    }

    #[test]
    fn test_format_timestamp_2023_jan_01() {
        let ts = format_timestamp(Duration::from_secs(1672531200));
        assert_eq!(ts, "2023-01-01T00:00:00.000000000Z");
    }

    #[test]
    fn test_format_timestamp_leap_year_2020_jan_01() {
        let ts = format_timestamp(Duration::from_secs(1577836800));
        assert_eq!(ts, "2020-01-01T00:00:00.000000000Z");
    }

    #[test]
    fn test_format_timestamp_year_end_boundary() {
        let ts = format_timestamp(Duration::from_secs(1704067199));
        assert_eq!(ts, "2023-12-31T23:59:59.000000000Z");
    }

    #[test]
    fn test_format_timestamp_with_nanos() {
        let dur = Duration::new(0, 123456789);
        let ts = format_timestamp(dur);
        assert_eq!(ts, "1970-01-01T00:00:00.123456789Z");
    }

    #[test]
    fn test_iso_timestamp_valid_format() {
        let ts = iso_timestamp();
        assert_eq!(ts.len(), 30, "iso_timestamp produced invalid length: {}", ts);
        assert!(ts.ends_with('Z'), "iso_timestamp should end with Z: {}", ts);
        let parts: Vec<&str> = ts.splitn(2, 'T').collect();
        assert_eq!(parts.len(), 2, "iso_timestamp missing T separator: {}", ts);
        let date = parts[0];
        assert_eq!(date.len(), 10, "date part wrong length: {}", date);
        assert!(date.chars().nth(4).unwrap() == '-', "date format wrong: {}", date);
        let time = parts[1].strip_suffix('Z').unwrap();
        assert_eq!(time.len(), 18, "time part wrong length: {}", time);
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
    fn test_format_timestamp_century_non_leap() {
        let ts = format_timestamp(Duration::from_secs(4102444800));
        assert_eq!(ts, "2100-01-01T00:00:00.000000000Z");
    }

    // ---- Middleware isolation tests ----

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