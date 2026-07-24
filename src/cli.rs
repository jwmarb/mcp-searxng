use clap::{Parser, Subcommand, ValueEnum};

/// SearXNG CLI - Search and browse the web privately
#[derive(Parser, Debug)]
#[command(name = "searxng-cli", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// SearXNG instance URL
    #[arg(long, global = true, env = "SEARXNG_URL")]
    pub searxng_url: Option<String>,

    /// Server port for the MCP server
    #[arg(long, global = true, env = "SEARXNG_SERVER_PORT")]
    pub server_port: Option<u16>,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    /// Path to config file
    #[arg(long, global = true)]
    pub config: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Markdown,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Search via SearXNG
    Search(SearchArgs),

    /// Fetch and extract content from a URL
    Fetch(FetchArgs),

    /// Start the MCP server
    Serve,

    /// Navigate to a URL in a browser session
    Navigate(NavigateArgs),

    /// Take an accessibility snapshot
    Snapshot(SnapshotArgs),

    /// Click an element
    Click(ClickArgs),

    /// Fill a form field
    Fill(FillArgs),

    /// Evaluate JavaScript
    Evaluate(EvaluateArgs),

    /// Take a screenshot
    Screenshot(ScreenshotArgs),

    /// List or manage browser tabs
    Tabs(TabsArgs),

    /// List active browser instances
    Instances,

    /// Kill a browser instance
    Kill(KillArgs),

    /// Get session info
    SessionInfo(SessionInfoArgs),
}

#[derive(Parser, Debug)]
pub struct SearchArgs {
    /// Search query
    pub query: String,

    /// Search category
    #[arg(long)]
    pub category: Option<String>,

    /// Search language
    #[arg(long)]
    pub language: Option<String>,

    /// Time range filter
    #[arg(long, value_enum)]
    pub time_range: Option<TimeRange>,

    /// Safe search level
    #[arg(long)]
    pub safe: Option<bool>,

    /// Page number
    #[arg(long, default_value_t = 1)]
    pub page: u32,

    /// Maximum number of results
    #[arg(long)]
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum TimeRange {
    Day,
    Week,
    Month,
    Year,
}

#[derive(Parser, Debug)]
pub struct FetchArgs {
    /// URL to fetch
    pub url: String,

    /// Maximum characters to return
    #[arg(long)]
    pub max_chars: Option<usize>,

    /// Request timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,

    /// Use headless browser rendering
    #[arg(long)]
    pub render: bool,
}

#[derive(Parser, Debug)]
pub struct NavigateArgs {
    /// Browser session ID
    #[arg(long)]
    pub session: String,

    /// URL to navigate to
    pub url: String,
}

#[derive(Parser, Debug)]
pub struct SnapshotArgs {
    /// Browser session ID
    #[arg(long)]
    pub session: String,
}

#[derive(Parser, Debug)]
pub struct ClickArgs {
    /// Browser session ID
    #[arg(long)]
    pub session: String,

    /// Element selector
    pub selector: String,
}

#[derive(Parser, Debug)]
pub struct FillArgs {
    /// Browser session ID
    #[arg(long)]
    pub session: String,

    /// Element selector
    pub selector: String,

    /// Value to fill
    pub value: String,
}

#[derive(Parser, Debug)]
pub struct EvaluateArgs {
    /// Browser session ID
    #[arg(long)]
    pub session: String,

    /// JavaScript expression to evaluate
    pub js: String,
}

#[derive(Parser, Debug)]
pub struct ScreenshotArgs {
    /// Browser session ID
    #[arg(long)]
    pub session: String,

    /// Output file path
    #[arg(long)]
    pub file: Option<String>,

    /// Element selector to screenshot
    #[arg(long)]
    pub selector: Option<String>,
}

#[derive(Parser, Debug)]
pub struct TabsArgs {
    /// Browser session ID
    #[arg(long)]
    pub session: String,

    /// Action to perform
    #[arg(long)]
    pub action: Option<TabAction>,

    /// URL for new tab
    #[arg(long)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum TabAction {
    List,
    New,
    Close,
    Select,
}

impl TabAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::List => "list",
            Self::New => "new",
            Self::Close => "close",
            Self::Select => "select",
        }
    }
}

#[derive(Parser, Debug)]
pub struct KillArgs {
/// Browser session ID
    #[arg(long)]
    pub session: String,
}

#[derive(Parser, Debug)]
pub struct SessionInfoArgs {
    /// Browser session ID
    #[arg(long)]
    pub session: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_action_as_str_list() {
        assert_eq!(TabAction::List.as_str(), "list");
    }

    #[test]
    fn test_tab_action_as_str_new() {
        assert_eq!(TabAction::New.as_str(), "new");
    }

    #[test]
    fn test_tab_action_as_str_close() {
        assert_eq!(TabAction::Close.as_str(), "close");
    }

    #[test]
    fn test_tab_action_as_str_select() {
        assert_eq!(TabAction::Select.as_str(), "select");
    }

    #[test]
    fn test_search_all_options() {
        let cli = Cli::try_parse_from([
            "searxng-cli",
            "search",
            "query",
            "--category",
            "news",
            "--language",
            "en",
            "--time-range",
            "week",
            "--safe",
            "true",
            "--page",
            "2",
            "--max-results",
            "10",
        ])
        .unwrap();
        if let Command::Search(args) = &cli.command {
            assert_eq!(args.query, "query");
            assert_eq!(args.category, Some("news".to_string()));
            assert_eq!(args.language, Some("en".to_string()));
            assert_eq!(args.time_range, Some(TimeRange::Week));
            assert_eq!(args.safe, Some(true));
            assert_eq!(args.page, 2);
            assert_eq!(args.max_results, Some(10));
        } else {
            panic!("Expected Search command");
        }
    }

    #[test]
    fn test_fetch_all_options() {
        let cli = Cli::try_parse_from([
            "searxng-cli",
            "fetch",
            "https://example.com",
            "--max-chars",
            "500",
            "--timeout",
            "10",
            "--render",
        ])
        .unwrap();
        if let Command::Fetch(args) = &cli.command {
            assert_eq!(args.url, "https://example.com");
            assert_eq!(args.max_chars, Some(500));
            assert_eq!(args.timeout, Some(10));
            assert!(args.render);
        } else {
            panic!("Expected Fetch command");
        }
    }

    #[test]
    fn test_time_range_enum_valid() {
        for value in &["day", "week", "month", "year"] {
            let result = Cli::try_parse_from([
                "searxng-cli",
                "search",
                "query",
                "--time-range",
                value,
            ]);
            assert!(result.is_ok(), "Failed to parse time-range: {}", value);
        }
    }

    #[test]
    fn test_time_range_enum_invalid() {
        let result = Cli::try_parse_from([
            "searxng-cli",
            "search",
            "query",
            "--time-range",
            "invalid",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_format_global_option() {
        let cli = Cli::try_parse_from(["searxng-cli", "--format", "json", "search", "query"]).unwrap();
        assert_eq!(cli.format, OutputFormat::Json);

        let cli = Cli::try_parse_from(["searxng-cli", "--format", "text", "search", "query"]).unwrap();
        assert_eq!(cli.format, OutputFormat::Text);

        let cli = Cli::try_parse_from(["searxng-cli", "--format", "markdown", "search", "query"]).unwrap();
        assert_eq!(cli.format, OutputFormat::Markdown);

        let result = Cli::try_parse_from(["searxng-cli", "--format", "invalid", "search", "query"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_default_page() {
        let cli = Cli::try_parse_from(["searxng-cli", "search", "query"]).unwrap();
        if let Command::Search(args) = &cli.command {
            assert_eq!(args.page, 1);
        } else {
            panic!("Expected Search command");
        }
    }
}