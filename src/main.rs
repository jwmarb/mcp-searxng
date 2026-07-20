mod browser;
mod cli;
mod config;
mod error;
mod fetch;
mod search;
mod server;

use std::sync::Arc;
use base64::Engine;

use clap::Parser;
use tokio::signal;

use crate::cli::{Cli, Command, OutputFormat, TimeRange};
use crate::config::Config;
use crate::error::{CliError, Result};
use crate::search::{Search, SearchParams, OutputFormat as SearchOutputFormat};
use crate::fetch::{Fetcher, FetchParams, RenderMode};
use crate::browser::BrowserManager;
use crate::browser::pool::BrowserPoolHandle;
use crate::server::session::SessionManager;
use crate::server::routes::create_router;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = Config::load_with_path(cli.config.clone());

    if let Some(url) = cli.searxng_url {
        config.searxng_url = url;
    }
    if let Some(port) = cli.server_port {
        config.server_port = port;
    }

    match cli.command {
        Command::Search(args) => run_search(&config, &args, cli.format).await?,
        Command::Fetch(args) => run_fetch(&config, &args, cli.format).await?,
        Command::Serve => run_serve(&config).await?,
        Command::Navigate(args) => run_navigate(&args).await?,
        Command::Snapshot(args) => run_snapshot(&args).await?,
        Command::Click(args) => run_click(&args).await?,
        Command::Fill(args) => run_fill(&args).await?,
        Command::Evaluate(args) => run_evaluate(&args).await?,
        Command::Screenshot(args) => run_screenshot(&args).await?,
        Command::Tabs(args) => run_tabs(&args).await?,
        Command::Instances => run_instances().await?,
        Command::Kill(args) => run_kill(&args).await?,
    }

    Ok(())
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
    println!("{}", output);

    Ok(())
}

async fn run_fetch(config: &Config, args: &crate::cli::FetchArgs, _format: OutputFormat) -> Result<()> {
    let fetcher = Fetcher::new().with_config(config.clone());

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
    println!("{}", serde_json::to_string_pretty(&response)?);

    Ok(())
}

async fn run_serve(config: &Config) -> Result<()> {
    let browser_manager = Arc::new(BrowserManager::new());
    browser_manager.launch(config.chrome_path.as_deref()).await?;

    let pool = BrowserPoolHandle::new(browser_manager.clone());
    let session_manager = SessionManager::new(pool);

    let app = create_router(session_manager);

    let addr = format!("127.0.0.1:{}", config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    println!("Server listening on {}", addr);

    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                eprintln!("Server error: {}", e);
            }
        }
        _ = signal::ctrl_c() => {
            println!("Shutting down...");
            let _ = browser_manager.shutdown().await;
        }
    }

    Ok(())
}

async fn run_navigate(args: &crate::cli::NavigateArgs) -> Result<()> {
    let config = Config::load();
    let session_id = args.id.as_deref().unwrap_or("default");
    let server_url = format!("http://127.0.0.1:{}", config.server_port);

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/navigate", server_url))
        .json(&serde_json::json!({
            "session": session_id,
            "url": &args.url
        }))
        .send()
        .await?;

    if response.status().is_success() {
        println!("Navigation successful");
        Ok(())
    } else {
        Err(CliError::Browser(format!("Server error: {}", response.status())))
    }
}

async fn run_snapshot(args: &crate::cli::SnapshotArgs) -> Result<()> {
    let config = Config::load();
    let session_id = args.id.as_deref().unwrap_or("default");
    let server_url = format!("http://127.0.0.1:{}", config.server_port);

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/snapshot?session={}", server_url, session_id))
        .send()
        .await?;

    if response.status().is_success() {
        let body: serde_json::Value = response.json().await?;
        let content = body["content"].as_str().unwrap_or("");
        println!("{}", content);
        Ok(())
    } else {
        Err(CliError::Browser(format!("Server error: {}", response.status())))
    }
}

async fn run_click(args: &crate::cli::ClickArgs) -> Result<()> {
    let config = Config::load();
    let session_id = args.id.as_deref().unwrap_or("default");
    let server_url = format!("http://127.0.0.1:{}", config.server_port);

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/click", server_url))
        .json(&serde_json::json!({
            "session": session_id,
            "selector": &args.selector
        }))
        .send()
        .await?;

    if response.status().is_success() {
        println!("Click successful");
        Ok(())
    } else {
        Err(CliError::Browser(format!("Server error: {}", response.status())))
    }
}

async fn run_fill(args: &crate::cli::FillArgs) -> Result<()> {
    let config = Config::load();
    let session_id = args.id.as_deref().unwrap_or("default");
    let server_url = format!("http://127.0.0.1:{}", config.server_port);

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/fill", server_url))
        .json(&serde_json::json!({
            "session": session_id,
            "selector": &args.selector,
            "value": &args.value
        }))
        .send()
        .await?;

    if response.status().is_success() {
        println!("Fill successful");
        Ok(())
    } else {
        Err(CliError::Browser(format!("Server error: {}", response.status())))
    }
}

async fn run_evaluate(args: &crate::cli::EvaluateArgs) -> Result<()> {
    let config = Config::load();
    let session_id = args.id.as_deref().unwrap_or("default");
    let server_url = format!("http://127.0.0.1:{}", config.server_port);

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/evaluate", server_url))
        .json(&serde_json::json!({
            "session": session_id,
            "script": &args.js
        }))
        .send()
        .await?;

    if response.status().is_success() {
        let body: serde_json::Value = response.json().await?;
        let result = &body["result"];
        println!("{}", result);
        Ok(())
    } else {
        Err(CliError::Browser(format!("Server error: {}", response.status())))
    }
}

async fn run_screenshot(args: &crate::cli::ScreenshotArgs) -> Result<()> {
    let config = Config::load();
    let session_id = args.id.as_deref().unwrap_or("default");
    let server_url = format!("http://127.0.0.1:{}", config.server_port);

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/screenshot?session={}", server_url, session_id))
        .send()
        .await?;

    if response.status().is_success() {
        if let Some(file_path) = &args.file {
            let bytes = response.bytes().await?;
            std::fs::write(file_path, &bytes)?;
            println!("Screenshot saved to {}", file_path);
        } else {
            let bytes = response.bytes().await?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            println!("{}", encoded);
        }
        Ok(())
    } else {
        Err(CliError::Browser(format!("Server error: {}", response.status())))
    }
}

async fn run_tabs(args: &crate::cli::TabsArgs) -> Result<()> {
    let config = Config::load();
    let session_id = args.id.as_deref().unwrap_or("default");
    let server_url = format!("http://127.0.0.1:{}", config.server_port);

    let mut payload = serde_json::json!({
        "session": session_id,
    });

    if let Some(ref action) = args.action {
        payload["action"] = serde_json::json!(action.as_str());
    }
    if let Some(ref url) = args.url {
        payload["url"] = serde_json::json!(url);
    }

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/tabs", server_url))
        .json(&payload)
        .send()
        .await?;

    if response.status().is_success() {
        let body: serde_json::Value = response.json().await?;
        println!("{}", serde_json::to_string_pretty(&body)?);
        Ok(())
    } else {
        Err(CliError::Browser(format!("Server error: {}", response.status())))
    }
}

async fn run_instances() -> Result<()> {
    let config = Config::load();
    let server_url = format!("http://127.0.0.1:{}", config.server_port);

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/api/instances", server_url))
        .send()
        .await?;

    if response.status().is_success() {
        let body: serde_json::Value = response.json().await?;
        println!("{}", serde_json::to_string_pretty(&body)?);
        Ok(())
    } else {
        Err(CliError::Browser(format!("Server error: {}", response.status())))
    }
}

async fn run_kill(args: &crate::cli::KillArgs) -> Result<()> {
    let config = Config::load();
    let server_url = format!("http://127.0.0.1:{}", config.server_port);

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/kill", server_url))
        .json(&serde_json::json!({
            "session": &args.id
        }))
        .send()
        .await?;

    if response.status().is_success() {
        println!("Session {} killed", args.id);
        Ok(())
    } else {
        Err(CliError::Browser(format!("Server error: {}", response.status())))
    }
}