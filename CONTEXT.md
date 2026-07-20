# CONTEXT.md - SearXNG CLI Domain Glossary

## Overview
SearXNG CLI is a Rust-based command-line tool for searching the web through SearXNG, fetching web page content, and automating browser interactions. It's designed for LLM agents that need internet access.

## Key Terms

### SearXNG Instance
A running SearXNG server that aggregates search results from multiple search engines. The CLI connects to this instance via HTTP API.

### Search Query
The text string passed to SearXNG for searching. Supports advanced syntax like `site:github.com` for engine-specific queries.

### Search Category
A filter for the type of results: `general`, `news`, `images`, `videos`, `it`, `science`, `files`, `social media`.

### Browser Session
An isolated browser context managed by the CLI server. Each session has its own cookies, storage, and pages (tabs).

### Session ID
A unique identifier for a browser session. Defaults to "default" if not specified with `--id`.

### Hybrid Fetch
A fetching strategy that first tries lightweight HTTP + HTML-to-markdown conversion, falling back to headless browser rendering for JavaScript-heavy pages.

### Server Mode
The CLI can run as a local HTTP server (`searxng-cli serve`) to manage browser automation sessions. Browser commands communicate with this server.

## Architecture

### Components
- **CLI Client**: Parses commands and arguments, makes HTTP requests to SearXNG and the local server
- **Local Server**: Manages Playwright browser sessions, provides REST API for browser operations
- **SearXNG Backend**: External search engine aggregation service

### Data Flow
1. User runs `searxng-cli search "query"`
2. CLI makes HTTP request to SearXNG instance
3. SearXNG returns JSON response with results
4. CLI formats and displays results

### Configuration
- **Config File**: `~/.config/searxng-cli/config.yaml`
- **Environment Variables**: `SEARXNG_URL`, `SEARXNG_SERVER_PORT`, `SEARXNG_CHROME_PATH`
- **CLI Flags**: Override config and env vars

## Error Handling
- All errors go to stderr
- Exit code 0 = success, 1 = error
- JSON output format for programmatic consumption