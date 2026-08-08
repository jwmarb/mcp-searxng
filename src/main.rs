mod browser;
mod browser_client;
mod chromium_download;
mod cli;
mod config;
mod error;
mod fetch;
mod response;
mod retry;
mod search;
mod server;
mod time;

use std::process;
use std::sync::Arc;

use clap::Parser;
use tokio::signal;

use browser_client::BrowserClient;
use crate::cli::{Cli, Command, OutputFormat, TimeRange};
use crate::chromium_download::resolve_chrome_path;
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::search::{Search, SearchParams, OutputFormat as SearchOutputFormat};
use crate::fetch::{Fetcher, FetchParams, RenderMode};
use crate::browser::BrowserManager;
use crate::browser::pool::BrowserPoolHandle;
use crate::retry::RetryClient;
use crate::server::session::SessionManager;
use crate::server::routes::create_router;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let mut config = Config::load_with_path(cli.config.clone());

    if let Some(url) = cli.searxng_url {
        config.searxng_url = url;
    }
    if let Some(port) = cli.server_port {
        config.server_port = port;
    }

    let result = match cli.command {
        Command::Search(args) => run_search(&config, &args, cli.format).await,
        Command::Fetch(args) => run_fetch(&config, &args, cli.format).await,
        Command::Serve => run_serve(&config).await,
        command => {
            let client = BrowserClient::new(&config);
            run_browser_command(&client, command, cli.format).await
        }
    };

    if let Err(err) = result {
        eprintln!("Error: {err}");
        let exit_code = err.error_code().exit_code();
        process::exit(exit_code);
    }
}

async fn run_browser_command(client: &BrowserClient, command: Command, format: OutputFormat) -> Result<()> {
    match command {
        Command::Navigate(args) => {
            client.navigate(&args.session, &args.url).await?;
            println!("Navigation successful");
        }
        Command::Snapshot(args) => {
            let html = client.snapshot(&args.session).await?;
            match format {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "html": html
                    }))?);
                }
                OutputFormat::Text => {
                    let md = html_to_markdown_rs::convert(&html, None)
                        .unwrap_or_default()
                        .content
                        .unwrap_or_default();
                    println!("{}", md);
                }
                OutputFormat::Markdown => {
                    let md = html_to_markdown_rs::convert(&html, None)
                        .unwrap_or_default()
                        .content
                        .unwrap_or_default();
                    println!("{}", md);
                }
            }
        }
        Command::Click(args) => {
            client.click(&args.session, &args.selector).await?;
            println!("Click successful");
        }
        Command::Fill(args) => {
            client.fill(&args.session, &args.selector, &args.value).await?;
            println!("Fill successful");
        }
        Command::Evaluate(args) => {
            let result = client.evaluate(&args.session, &args.js).await?;
            println!("{result}");
        }
        Command::Screenshot(args) => {
            client.screenshot(&args.session, args.file.as_deref()).await?;
        }
        Command::Tabs(args) => {
            let body = client.tabs(&args.session, args.action.as_ref().map(|a| a.as_str()), args.url.as_deref()).await?;
            let data = body.get("data").unwrap_or(&body);
            print_value(data, &format);
        }
        Command::Instances => {
            let body = client.instances().await?;
            let data = body.get("data").unwrap_or(&body);
            print_value(data, &format);
        }
        Command::Kill(args) => {
            client.kill(&args.session).await?;
            println!("Session {} killed", args.session);
        }
        Command::SessionInfo(args) => {
            let info = client.session_info(&args.session).await?;
            let data = info.get("data").unwrap_or(&info);
            print_value(data, &format);
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn print_value(value: &serde_json::Value, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
        }
        OutputFormat::Text => print_text(value),
        OutputFormat::Markdown => print_markdown(value),
    }
}

fn print_text(value: &serde_json::Value) {
    match value {
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                println!("No results");
                return;
            }
            for (i, item) in arr.iter().enumerate() {
                match item {
                    serde_json::Value::Object(obj) => {
                        if obj.contains_key("title") && obj.contains_key("url") {
                            println!("{}. {} ({})", i + 1,
                                obj.get("title").and_then(|v| v.as_str()).unwrap_or("N/A"),
                                obj.get("url").and_then(|v| v.as_str()).unwrap_or("N/A"));
                        } else if obj.contains_key("id") || obj.contains_key("session") {
                            let id = obj.get("id").or(obj.get("session"))
                                .and_then(|v| v.as_str()).unwrap_or("unknown");
                            let url = obj.get("active_url").and_then(|v| v.as_str()).unwrap_or("N/A");
                            println!("{}. {} ({})", i + 1, id, url);
                        } else {
                            println!("{}. {}", i + 1, serde_json::to_string(item).unwrap_or_default());
                        }
                    }
                    _ => println!("{}. {}", i + 1, item),
                }
            }
        }
        serde_json::Value::Object(obj) => {
            for (key, val) in obj.iter() {
                match val {
                    serde_json::Value::String(s) => println!("{}: {}", key, s),
                    serde_json::Value::Number(n) => println!("{}: {}", key, n),
                    serde_json::Value::Bool(b) => println!("{}: {}", key, b),
                    serde_json::Value::Null => println!("{}: (null)", key),
                    _ => {}
                }
            }
        }
        _ => println!("{}", serde_json::to_string_pretty(value).unwrap_or_default()),
    }
}

fn print_markdown(value: &serde_json::Value) {
    match value {
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                println!("No results");
                return;
            }
            for item in arr.iter() {
                match item {
                    serde_json::Value::Object(obj) => {
                        if obj.contains_key("title") && obj.contains_key("url") {
                            let title = obj.get("title").and_then(|v| v.as_str()).unwrap_or("N/A");
                            let url = obj.get("url").and_then(|v| v.as_str()).unwrap_or("N/A");
                            println!("- [{}]({})", title, url);
                        } else if obj.contains_key("id") || obj.contains_key("session") {
                            let id = obj.get("id").or(obj.get("session"))
                                .and_then(|v| v.as_str()).unwrap_or("unknown");
                            let url = obj.get("active_url").and_then(|v| v.as_str()).unwrap_or("N/A");
                            println!("- **{}**: {}", id, url);
                        } else {
                            println!("- {}", serde_json::to_string(item).unwrap_or_default());
                        }
                    }
                    _ => println!("- {}", item),
                }
            }
        }
        serde_json::Value::Object(obj) => {
            for (key, val) in obj.iter() {
                match val {
                    serde_json::Value::String(s) => println!("**{}**: {}", key, s),
                    serde_json::Value::Number(n) => println!("**{}**: {}", key, n),
                    serde_json::Value::Bool(b) => println!("**{}**: {}", key, b),
                    serde_json::Value::Null => println!("**{}**: *(null)*", key),
                    _ => {}
                }
            }
        }
        _ => println!("{}", serde_json::to_string_pretty(value).unwrap_or_default()),
    }
}

async fn run_search(config: &Config, args: &crate::cli::SearchArgs, format: OutputFormat) -> Result<()> {
    let search = Search::new(config);

    let output_format = match format {
        OutputFormat::Json => SearchOutputFormat::Json,
        OutputFormat::Text => SearchOutputFormat::Text,
        OutputFormat::Markdown => SearchOutputFormat::Markdown,
    };

    let params = SearchParams {
        query: args.query.clone(),
        categories: args.category.clone(),
        language: args.language.clone(),
        time_range: args.time_range.as_ref().map(|t| match t {
            TimeRange::Day => "day".to_string(),
            TimeRange::Week => "week".to_string(),
            TimeRange::Month => "month".to_string(),
            TimeRange::Year => "year".to_string(),
        }),
safesearch: args.safe.map(|s| if s { 1 } else { 0 }),
        page: Some(args.page),
        max_results: args.max_results,
        format: output_format.clone(),
    };

    let response = search.search(&params).await?;
    let output = Search::format_response(&response, output_format);
    println!("{output}");

    Ok(())
}

async fn run_fetch(config: &Config, args: &crate::cli::FetchArgs, format: OutputFormat) -> Result<()> {
    let retry_client = RetryClient::new(&config.retry);
    let fetcher = Fetcher::new(retry_client).with_config(config.clone());

    let params = FetchParams {
        url: args.url.clone(),
        max_chars: args.max_chars,
        timeout: args.timeout,
        render_mode: if args.render {
            RenderMode::Render
        } else {
            RenderMode::Lightweight
        },
    };

    let response = fetcher.fetch(&params).await?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        OutputFormat::Text => {
            println!("Title: {}", response.title);
            println!("URL: {}", response.url);
            println!("Status: {}", response.status_code);
            if let Some(max) = args.max_chars {
                let content = crate::fetch::util::truncate_content(&response.content, max);
                println!("\n{}", content);
            } else {
                println!("\n{}", response.content);
            }
        }
        OutputFormat::Markdown => {
            println!("{} ({})", response.title, response.url);
            println!("Status: {}", response.status_code);
            if let Some(max) = args.max_chars {
                let content = crate::fetch::util::truncate_content(&response.content, max);
                println!("\n{}", content);
            } else {
                println!("\n{}", response.content);
            }
        }
    }

    Ok(())
}

async fn run_serve(config: &Config) -> Result<()> {
    let chrome_path = resolve_chrome_path(config.chrome_path.as_deref()).await?;

    let browser_manager = Arc::new(BrowserManager::new());
    browser_manager.launch(Some(&chrome_path)).await?;

    let pool = BrowserPoolHandle::new(
        browser_manager.clone(),
        config.max_sessions,
        config.session_idle_timeout_secs,
    );
    let session_manager = SessionManager::new(pool);

    let search = Arc::new(Search::new(config));
    let config_arc = Arc::new(config.clone());

    let app = create_router(session_manager, search, config_arc);

    let addr = format!("127.0.0.1:{port}", port = config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    println!("Server listening on {addr}");

    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                eprintln!("Server error: {e}");
            }
        }
        _ = signal::ctrl_c() => {
            println!("Shutting down...");
            let _ = browser_manager.shutdown().await;
        }
    }

    Ok(())
}