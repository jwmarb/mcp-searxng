use clap::Parser;
use searxng_cli::cli::Cli;

#[test]
fn test_navigate_missing_session_fails() {
    let result = Cli::try_parse_from(["searxng-cli", "navigate", "https://example.com"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("session"));
}

#[test]
fn test_navigate_with_session_succeeds() {
    let result = Cli::try_parse_from(["searxng-cli", "navigate", "--session", "abc123", "https://example.com"]);
    assert!(result.is_ok());
}

#[test]
fn test_snapshot_missing_session_fails() {
    let result = Cli::try_parse_from(["searxng-cli", "snapshot"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("session"));
}

#[test]
fn test_click_missing_session_fails() {
    let result = Cli::try_parse_from(["searxng-cli", "click", "#button"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("session"));
}

#[test]
fn test_session_info_parses() {
    let result = Cli::try_parse_from(["searxng-cli", "session-info", "--session", "abc123"]);
    assert!(result.is_ok());
    let cli = result.unwrap();
    match cli.command {
        searxng_cli::cli::Command::SessionInfo(args) => {
            assert_eq!(args.session, "abc123");
        }
        _ => panic!("Expected SessionInfo command"),
    }
}

#[test]
fn test_session_info_missing_session_fails() {
    let result = Cli::try_parse_from(["searxng-cli", "session-info"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("session"));
}

#[test]
fn test_fill_missing_session_fails() {
    let result = Cli::try_parse_from(["searxng-cli", "fill", "username", "john"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("session"));
}

#[test]
fn test_fill_with_session_succeeds() {
    let result = Cli::try_parse_from(["searxng-cli", "fill", "--session", "abc123", "username", "john"]);
    assert!(result.is_ok());
}

#[test]
fn test_evaluate_missing_session_fails() {
    let result = Cli::try_parse_from(["searxng-cli", "evaluate", "1+1"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("session"));
}

#[test]
fn test_evaluate_with_session_succeeds() {
    let result = Cli::try_parse_from(["searxng-cli", "evaluate", "--session", "abc123", "1+1"]);
    assert!(result.is_ok());
}

#[test]
fn test_screenshot_missing_session_fails() {
    let result = Cli::try_parse_from(["searxng-cli", "screenshot"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("session"));
}

#[test]
fn test_screenshot_with_session_succeeds() {
    let result = Cli::try_parse_from(["searxng-cli", "screenshot", "--session", "abc123"]);
    assert!(result.is_ok());
}

#[test]
fn test_tabs_missing_session_fails() {
    let result = Cli::try_parse_from(["searxng-cli", "tabs", "list"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("session"));
}

#[test]
fn test_tabs_with_session_and_action_succeeds() {
    let result = Cli::try_parse_from(["searxng-cli", "tabs", "--session", "abc123", "--action", "list"]);
    assert!(result.is_ok());
}

#[test]
fn test_instances_command() {
    let result = Cli::try_parse_from(["searxng-cli", "instances"]);
    assert!(result.is_ok());
}

#[test]
fn test_kill_missing_session_fails() {
    let result = Cli::try_parse_from(["searxng-cli", "kill"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("session"));
}

#[test]
fn test_kill_with_session_succeeds() {
    let result = Cli::try_parse_from(["searxng-cli", "kill", "--session", "abc123"]);
    assert!(result.is_ok());
}