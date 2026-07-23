use std::process::Command;
use std::sync::Arc;

use searxng_cli::browser::{BrowserPoolHandle, BrowserManager};
use searxng_cli::config::Config;
use searxng_cli::search::Search;
use searxng_cli::server::session::SessionManager;
use searxng_cli::server::routes::create_router;

// ---------------------------------------------------------------------------
// CLI smoke tests
// ---------------------------------------------------------------------------

#[test]
fn test_help_command() {
    let mut cmd = Command::new("cargo");
    let output = cmd.args(&["run", "--bin", "searxng-cli", "--", "--help"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SearXNG CLI"));
}

#[test]
fn test_version_command() {
    let mut cmd = Command::new("cargo");
    let output = cmd.args(&["run", "--bin", "searxng-cli", "--", "--version"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("searxng-cli"));
}

#[test]
fn test_invalid_command() {
    let mut cmd = Command::new("cargo");
    let output = cmd.args(&["run", "--bin", "searxng-cli", "--", "invalid-command"]).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error") || stderr.contains("invalid"));
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a SessionManager backed by a BrowserPoolHandle whose browser has
/// NOT been launched.  Good enough for testing routing, deserialization,
/// and the "session not found" paths.
fn make_session_manager() -> SessionManager {
    let manager = BrowserManager::new();
    let pool = BrowserPoolHandle::new(Arc::new(manager), 8, 600);
    SessionManager::new(pool)
}

/// Start the axum server on a random port and return the base URL.
/// The server task is returned so it can be joined when the test finishes.
async fn start_server(session_manager: SessionManager) -> (String, tokio::task::JoinHandle<()>) {
    let search = Arc::new(Search::new(&Config::default()));
    let config = Arc::new(Config::default());
    let router = create_router(session_manager, search, config);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}");
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (base, task)
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health_check_returns_200() {
    let sm = make_session_manager();
    let (base, task) = start_server(sm).await;

    let resp = reqwest::get(format!("{base}/api/health")).await.unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["success"], true);

    task.abort();
}

// ---------------------------------------------------------------------------
// Navigate (creates a new session)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore] // requires Playwright
async fn test_navigate_creates_session() {
    let sm = make_session_manager();
    let (base, task) = start_server(sm).await;

    let resp = reqwest::Client::new()
        .post(format!("{base}/api/navigate"))
        .json(&serde_json::json!({
            "session": "e2e-test",
            "url": "data:text/html,<h1>hello</h1>"
        }))
        .send()
        .await
        .unwrap();

    // With a real browser this returns 200 + {"ok": true, "data": null}
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["success"], true);

    task.abort();
}

// ---------------------------------------------------------------------------
// Snapshot (returns "data" field)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore] // requires Playwright
async fn test_snapshot_returns_data_field() {
    let sm = make_session_manager();
    let (base, task) = start_server(sm).await;

    // Navigate first to create the session
    reqwest::Client::new()
        .post(format!("{base}/api/navigate"))
        .json(&serde_json::json!({
            "session": "snap-test",
            "url": "data:text/html,<h1>hello</h1>"
        }))
        .send()
        .await
        .unwrap();

    let resp = reqwest::get(format!("{base}/api/snapshot?session=snap-test"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["success"], true);
    // The respond() wrapper uses "data", not "content" or "snapshot"
    assert!(body.get("data").is_some(), "response must contain 'data' field");

    task.abort();
}

// ---------------------------------------------------------------------------
// Evaluate (returns "data" field with result)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore] // requires Playwright
async fn test_evaluate_returns_data_field() {
    let sm = make_session_manager();
    let (base, task) = start_server(sm).await;

    // Navigate first
    reqwest::Client::new()
        .post(format!("{base}/api/navigate"))
        .json(&serde_json::json!({
            "session": "eval-test",
            "url": "data:text/html,<h1>hello</h1>"
        }))
        .send()
        .await
        .unwrap();

    let resp = reqwest::Client::new()
        .post(format!("{base}/api/evaluate"))
        .json(&serde_json::json!({
            "session": "eval-test",
            "script": "1 + 1"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert!(body.get("data").is_some(), "response must contain 'data' field");
    // The actual JS result should be 2
    assert_eq!(body["data"], 2);

    task.abort();
}

// ---------------------------------------------------------------------------
// Session info (ISO-8601 timestamps)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore] // requires Playwright to create a real session
async fn test_session_info_returns_iso8601_timestamps() {
    let sm = make_session_manager();
    let (base, task) = start_server(sm).await;

    // Create session via navigate
    reqwest::Client::new()
        .post(format!("{base}/api/navigate"))
        .json(&serde_json::json!({
            "session": "info-test",
            "url": "data:text/html,<h1>hello</h1>"
        }))
        .send()
        .await
        .unwrap();

    let resp = reqwest::get(format!("{base}/api/session/info-test"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["id"], "info-test");

    // created_at must be a valid ISO-8601 timestamp
    let created_at = body["data"]["created_at"].as_str().unwrap();
    assert!(
        created_at.ends_with('Z'),
        "created_at should end with Z: {created_at}"
    );
    assert!(
        created_at.contains('T'),
        "created_at should contain T separator: {created_at}"
    );

    task.abort();
}

// ---------------------------------------------------------------------------
// Session info – 404 for unknown session (no browser needed)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_info_404_unknown_session() {
    let sm = make_session_manager();
    let (base, task) = start_server(sm).await;

    let resp = reqwest::get(format!("{base}/api/session/nonexistent"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some());

    task.abort();
}

// ---------------------------------------------------------------------------
// Kill session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_kill_session_not_found() {
    let sm = make_session_manager();
    let (base, task) = start_server(sm).await;

    let resp = reqwest::Client::new()
        .post(format!("{base}/api/kill"))
        .json(&serde_json::json!({
            "session": "does-not-exist"
        }))
        .send()
        .await
        .unwrap();

    // respond() maps SessionNotFound → 404
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some());

    task.abort();
}

#[tokio::test]
#[ignore] // requires Playwright to create a real session first
async fn test_kill_session_removes_session() {
    let sm = make_session_manager();
    let (base, task) = start_server(sm).await;

    // Create session
    reqwest::Client::new()
        .post(format!("{base}/api/navigate"))
        .json(&serde_json::json!({
            "session": "kill-test",
            "url": "data:text/html,<h1>hello</h1>"
        }))
        .send()
        .await
        .unwrap();

    // Verify session exists
    let resp = reqwest::get(format!("{base}/api/session/kill-test"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Kill it
    let resp = reqwest::Client::new()
        .post(format!("{base}/api/kill"))
        .json(&serde_json::json!({
            "session": "kill-test"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["success"], true);

    // Verify session is gone
    let resp = reqwest::get(format!("{base}/api/session/kill-test"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    task.abort();
}

// ---------------------------------------------------------------------------
// Instances (returns list of sessions)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_instances_returns_empty_list() {
    let sm = make_session_manager();
    let (base, task) = start_server(sm).await;

    let resp = reqwest::get(format!("{base}/api/instances")).await.unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    // No sessions created, so empty array
    assert!(body["data"].is_array());

    task.abort();
}

#[tokio::test]
#[ignore] // requires Playwright
async fn test_instances_returns_session_list() {
    let sm = make_session_manager();
    let (base, task) = start_server(sm).await;

    reqwest::Client::new()
        .post(format!("{base}/api/navigate"))
        .json(&serde_json::json!({
            "session": "inst-test",
            "url": "data:text/html,<h1>hello</h1>"
        }))
        .send()
        .await
        .unwrap();

    let resp = reqwest::get(format!("{base}/api/instances")).await.unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let sessions = body["data"].as_array().unwrap();
    assert!(!sessions.is_empty(), "sessions list should not be empty");
    assert!(
        sessions.iter().any(|s| s["id"] == "inst-test"),
        "inst-test session should be in the list"
    );

    task.abort();
}