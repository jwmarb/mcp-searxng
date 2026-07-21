use serde::Serialize;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Metadata attached to every response envelope.
#[derive(Debug, Clone, Serialize)]
pub struct ResponseMetadata {
    pub duration_ms: u64,
    pub timestamp: String,
}

impl ResponseMetadata {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration_ms: duration.as_millis() as u64,
            timestamp: format_timestamp(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO)),
        }
    }
}

/// Wraps all CLI output in a consistent JSON envelope.
#[derive(Debug, Clone, Serialize)]
pub struct ResponseEnvelope<T: Serialize> {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ResponseMetadata>,
}

impl<T: Serialize> ResponseEnvelope<T> {
    /// Create a success envelope with data and metadata.
    pub fn success(data: T, started_at: Instant) -> Self {
        let elapsed = started_at.elapsed();
        Self {
            status: "success".to_string(),
            data: Some(data),
            error: None,
            metadata: Some(ResponseMetadata::new(elapsed)),
        }
    }

    /// Create an error envelope (no data, includes timing metadata).
    pub fn error(msg: impl Into<String>, started_at: Instant) -> Self {
        let elapsed = started_at.elapsed();
        Self {
            status: "error".to_string(),
            data: None,
            error: Some(msg.into()),
            metadata: Some(ResponseMetadata::new(elapsed)),
        }
    }
}

/// Format a Duration since UNIX_EPOCH as an ISO-8601 timestamp (no external crate).
fn format_timestamp(dur: Duration) -> String {
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
        if d < yd {
            break;
        }
        d -= yd;
        y += 1;
    }

    let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    let months = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 1u32;
    let mut day = d as u32 + 1;
    for &mlen in &months {
        if day <= mlen {
            break;
        }
        day -= mlen;
        m += 1;
    }

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        y, m, day, hh, mm, ss, nanos
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_envelope_serializes() {
        let started_at = Instant::now();
        let envelope: ResponseEnvelope<String> = ResponseEnvelope::success("hello".to_string(), started_at);

        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains(r#""status":"success""#));
        assert!(json.contains(r#""data":"hello""#));
        assert!(json.contains(r#""metadata""#));
        assert!(json.contains(r#""duration_ms""#));
        assert!(json.contains(r#""timestamp""#));
        assert!(!json.contains(r#""error""#));
    }

    #[test]
    fn test_error_envelope_serializes() {
        let started_at = Instant::now();
        let envelope: ResponseEnvelope<String> = ResponseEnvelope::error("something went wrong", started_at);

        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains(r#""status":"error""#));
        assert!(json.contains(r#""error":"something went wrong""#));
        assert!(json.contains(r#""metadata""#));
        assert!(!json.contains(r#""data""#));
    }

    #[test]
    fn test_metadata_new() {
        let meta = ResponseMetadata::new(Duration::from_millis(150));
        assert_eq!(meta.duration_ms, 150);
        assert!(meta.timestamp.contains('T'));
        assert!(meta.timestamp.ends_with('Z'));
    }

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
    fn test_format_timestamp_with_nanos() {
        let dur = Duration::new(0, 123_000_000);
        let ts = format_timestamp(dur);
        assert_eq!(ts, "1970-01-01T00:00:00.123000000Z");
    }

    #[test]
    fn test_envelope_clone() {
        let started_at = Instant::now();
        let envelope: ResponseEnvelope<String> = ResponseEnvelope::success("test".to_string(), started_at);
        let cloned = envelope.clone();
        let json_orig = serde_json::to_string(&envelope).unwrap();
        let json_clone = serde_json::to_string(&cloned).unwrap();
        assert_eq!(json_orig, json_clone);
    }
}