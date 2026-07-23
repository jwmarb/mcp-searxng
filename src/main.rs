mod browser;
mod browser_client;
mod cli;
mod config;
mod error;
mod fetch;
mod response;
mod retry;
mod search;
mod server;
mod time;

use std::sync::Arc;

use clap::Parser;
use tokio::signal;

use browser_client::BrowserClient;
use crate::cli::{Cli, Command, OutputFormat, TimeRange};
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
        Command::SessionInfo(args) => run_session_info(&args.session).await?,
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
    println!("{}", serde_json::to_string_pretty(&response)?);

    Ok(())
}

async fn run_serve(config: &Config) -> Result<()> {
    let browser_manager = Arc::new(BrowserManager::new());
    browser_manager.launch(config.chrome_path.as_deref()).await?;

    let pool = BrowserPoolHandle::new(
        browser_manager.clone(),
        config.max_sessions,
        config.session_idle_timeout_secs,
    );
    let session_manager = SessionManager::new(pool);

    let search = Arc::new(Search::new(config));
    let config_arc = Arc::new(config.clone());

    let app = create_router(session_manager, search, config_arc);

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
    let client = BrowserClient::new(&Config::load());
    let session_id = &args.session;
    client.navigate(session_id, &args.url).await?;
    println!("Navigation successful");
    Ok(())
}

async fn run_snapshot(args: &crate::cli::SnapshotArgs) -> Result<()> {
    let client = BrowserClient::new(&Config::load());
    let session_id = &args.session;
    let content = client.snapshot(session_id).await?;
    println!("{}", content);
    Ok(())
}

async fn run_click(args: &crate::cli::ClickArgs) -> Result<()> {
    let client = BrowserClient::new(&Config::load());
    let session_id = &args.session;
    client.click(session_id, &args.selector).await?;
    println!("Click successful");
    Ok(())
}

async fn run_fill(args: &crate::cli::FillArgs) -> Result<()> {
    let client = BrowserClient::new(&Config::load());
    let session_id = &args.session;
    client.fill(session_id, &args.selector, &args.value).await?;
    println!("Fill successful");
    Ok(())
}

async fn run_evaluate(args: &crate::cli::EvaluateArgs) -> Result<()> {
    let client = BrowserClient::new(&Config::load());
    let session_id = &args.session;
    let result = client.evaluate(session_id, &args.js).await?;
    println!("{}", result);
    Ok(())
}

async fn run_screenshot(args: &crate::cli::ScreenshotArgs) -> Result<()> {
    let client = BrowserClient::new(&Config::load());
    let session_id = &args.session;
    client.screenshot(session_id, args.file.as_deref()).await?;
    Ok(())
}

async fn run_tabs(args: &crate::cli::TabsArgs) -> Result<()> {
    let client = BrowserClient::new(&Config::load());
    let session_id = &args.session;
    let body = client.tabs(session_id, args.action.as_ref().map(|a| a.as_str()), args.url.as_deref()).await?;
    println!("{}", serde_json::to_string_pretty(&body as &serde_json::Value)?);
    Ok(())
}

async fn run_instances() -> Result<()> {
    let client = BrowserClient::new(&Config::load());
    let body = client.instances().await?;
    println!("{}", serde_json::to_string_pretty(&body as &serde_json::Value)?);
    Ok(())
}

async fn run_kill(args: &crate::cli::KillArgs) -> Result<()> {
    let client = BrowserClient::new(&Config::load());
    client.kill(&args.session).await?;
    println!("Session {} killed", args.session);
    Ok(())
}

async fn run_session_info(session: &str) -> Result<()> {
    let client = BrowserClient::new(&Config::load());
    let info = client.session_info(session).await?;
    println!("{}", serde_json::to_string_pretty(&info)?);
    Ok(())
}