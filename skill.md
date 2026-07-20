# SearXNG CLI Skill

## Overview
A CLI tool for searching the web via SearXNG, fetching page content, and browser automation. Designed for LLM agents to perform web research.

## Installation
```bash
# Install musl target for static linking (one-time)
rustup target add x86_64-unknown-linux-musl

# Build static binary (no DLLs / shared library dependencies)
cargo build --release --target x86_64-unknown-linux-musl

# Or use pre-built static binary from releases
```

## Quick Start

### Search the Web
```bash
searxng-cli search "your query" --max-results 5
```

### Fetch Page Content
```bash
searxng-cli fetch "https://example.com"
```

### Browser Automation (requires server)
```bash
# Start server
searxng-cli serve

# In another terminal
searxng-cli navigate --id session1 "https://example.com"
searxng-cli snapshot --id session1
```

#### Using tmux for background server
```bash
# Start server in tmux background session
tmux new-session -d -s searxng 'searxng-cli serve'

# Run commands (server runs in background)
searxng-cli navigate --id session1 "https://example.com"

# When done, kill the tmux session
tmux kill-session -t searxng
```

## Commands

### search
Search via SearXNG.

**Usage:**
```bash
searxng-cli search <QUERY> [OPTIONS]
```

**Options:**
- `--category <CATEGORY>` - Search category (general, news, images, videos, it, science, files, social media)
- `--language <LANGUAGE>` - Language code (en, de, fr, etc.)
- `--time-range <RANGE>` - Time filter (day, week, month, year)
- `--safe <BOOL>` - Safe search (true/false)
- `--page <NUMBER>` - Page number
- `--max-results <NUMBER>` - Maximum results to return
- `--format <FORMAT>` - Output format (json, text, markdown)

**Examples:**
```bash
# Basic search
searxng-cli search "Rust programming" --max-results 3

# News search
searxng-cli search "AI news" --category news --time-range week

# IT search
searxng-cli search "clap rust" --category it --format json
```

### fetch
Fetch and extract content from a URL.

**Usage:**
```bash
searxng-cli fetch <URL> [OPTIONS]
```

**Options:**
- `--max-chars <NUMBER>` - Maximum characters to return
- `--timeout <SECONDS>` - Request timeout
- `--render` - Use browser rendering for JS-heavy pages

**Examples:**
```bash
# Fetch page content
searxng-cli fetch "https://example.com"

# With character limit
searxng-cli fetch "https://example.com" --max-chars 5000
```

### serve
Start the browser automation server.

**Usage:**
```bash
searxng-cli serve [OPTIONS]
```

**Options:**
- `--server-port <PORT>` - Server port (default: 18960)

**Examples:**
```bash
# Start server
searxng-cli serve

# Custom port
searxng-cli serve --server-port 18961
```

### navigate
Navigate to a URL in a browser session.

**Usage:**
```bash
searxng-cli navigate <URL> [OPTIONS]
```

**Options:**
- `--id <SESSION_ID>` - Browser session ID (default: default)

### snapshot
Take an accessibility snapshot of the current page.

**Usage:**
```bash
searxng-cli snapshot [OPTIONS]
```

**Options:**
- `--id <SESSION_ID>` - Browser session ID

### click
Click an element on the page.

**Usage:**
```bash
searxng-cli click <SELECTOR> [OPTIONS]
```

**Options:**
- `--id <SESSION_ID>` - Browser session ID

### fill
Fill a form field.

**Usage:**
```bash
searxng-cli fill <SELECTOR> <VALUE> [OPTIONS]
```

**Options:**
- `--id <SESSION_ID>` - Browser session ID

### evaluate
Execute JavaScript on the page.

**Usage:**
```bash
searxng-cli evaluate <JAVASCRIPT> [OPTIONS]
```

**Options:**
- `--id <SESSION_ID>` - Browser session ID

### screenshot
Take a screenshot of the page.

**Usage:**
```bash
searxng-cli screenshot [OPTIONS]
```

**Options:**
- `--id <SESSION_ID>` - Browser session ID
- `--file <PATH>` - Output file path

### tabs
Manage browser tabs.

**Usage:**
```bash
searxng-cli tabs [OPTIONS]
```

**Options:**
- `--id <SESSION_ID>` - Browser session ID
- `--action <ACTION>` - Action (list, new, close, select)
- `--url <URL>` - URL for new tab

### instances
List active browser sessions.

**Usage:**
```bash
searxng-cli instances
```

### kill
Kill a browser session.

**Usage:**
```bash
searxng-cli kill <SESSION_ID>
```

## Configuration

### Config File
Location: `~/.config/searxng-cli/config.yaml`

```yaml
searxng_url: "http://localhost:8888"
server_port: 18960
chrome_path: null
```

### Environment Variables
- `SEARXNG_URL` - SearXNG instance URL
- `SEARXNG_SERVER_PORT` - Server port
- `SEARXNG_CHROME_PATH` - Path to Chromium binary

## Best Practices

### For LLM Agents
1. **Always use `--max-results`** to limit search results (recommended: 3-5)
2. **Use `--format json`** for programmatic parsing
3. **Start with search**, then fetch specific URLs
4. **Use browser mode only when necessary** (JS-heavy pages, interactive content)
5. **Kill browser sessions** when done to free resources

### Search Strategy
1. Start with a broad search
2. Identify relevant URLs from results
3. Fetch specific pages for detailed content
4. Use categories and filters to narrow results

### Error Handling
- Check exit codes (0 = success, 1 = error)
- Read stderr for error messages
- Retry failed requests with exponential backoff

## Troubleshooting

### Common Issues
1. **SearXNG not running**: Ensure SearXNG instance is accessible at configured URL
2. **Browser errors**: Install Chromium or set `chrome_path` in config
3. **Timeout errors**: Increase `--timeout` value for slow pages

### Debug Mode
```bash
RUST_LOG=debug searxng-cli search "query"
```