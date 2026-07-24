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
use crate::error::Result;
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
        command => {
            let client = BrowserClient::new(&config);
            run_browser_command(&client, command).await?;
        }
    }

    Ok(())
}

async fn run_browser_command(client: &BrowserClient, command: Command) -> Result<()> {
    match command {
        Command::Navigate(args) => {
            client.navigate(&args.session, &args.url).await?;
            println!("Navigation successful");
        }
        Command::Snapshot(args) => {
            let content = client.snapshot(&args.session).await?;
            println!("{content}");
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
            println!("{}", serde_json::to_string_pretty(&body as &serde_json::Value)?);
        }
        Command::Instances => {
            let body = client.instances().await?;
            println!("{}", serde_json::to_string_pretty(&body as &serde_json::Value)?);
        }
        Command::Kill(args) => {
            client.kill(&args.session).await?;
            println!("Session {} killed", args.session);
        }
        Command::SessionInfo(args) => {
            let info = client.session_info(&args.session).await?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        _ => unreachable!(),
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
    println!("{output}");

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