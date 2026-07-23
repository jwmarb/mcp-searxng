---
name: searxng-cli
description: For performing search queries on the internet like a real end-user. This skill provides usage tutorial on searxng-cli
---

## Overview

`searxng-cli` is a Rust CLI + axum HTTP server for web search, page content extraction, and headless browser automation via Playwright/CDP. Designed as the internet-access layer for LLM agents.

**Architecture**: Two modes of operation:

1. **Standalone CLI** — `search` and `fetch` work directly against SearXNG (no server needed)
2. **Client-Server** — Browser commands (`navigate`, `snapshot`, `click`, `fill`, `evaluate`, `screenshot`, `tabs`, `kill`, `instances`, `session-info`) require `searxng-cli serve` running

## Prerequisites

| Component           | Required For                            | Default URL                            |
| ------------------- | --------------------------------------- | -------------------------------------- |
| SearXNG instance    | `search` command                        | `http://localhost:8888`                |
| `searxng-cli serve` | All browser commands + `fetch --render` | `http://localhost:18960`               |
| Chromium/Chrome     | Browser automation                      | Auto-detected or `SEARXNG_CHROME_PATH` |

**Quick setup:**

```bash
# Start SearXNG (Docker)
docker compose -f docker-compose.dev.yaml up -d

# Start the browser server (separate terminal or tmux)
searxng-cli serve

# Now all commands work
```

## Command Reference

### search — Web Search via SearXNG

Queries SearXNG and returns results. Results are cached (TTL: 300s, max 200 entries).

```bash
searxng-cli search <QUERY> [OPTIONS]
```

| Option          | Type   | Default                   | Description                                                                                             |
| --------------- | ------ | ------------------------- | ------------------------------------------------------------------------------------------------------- |
| `--category`    | string | none (general)            | Search category: `general`, `news`, `images`, `videos`, `it`, `science`, `files`, `social media`, `map` |
| `--language`    | string | auto                      | Language code: `en`, `de`, `fr`, `es`, `zh`, etc.                                                       |
| `--time-range`  | enum   | none                      | `day`, `week`, `month`, `year`                                                                          |
| `--safe`        | bool   | none                      | Safe search: `true`/`false`                                                                             |
| `--page`        | u32    | 1                         | Pagination page number                                                                                  |
| `--max-results` | usize  | none (returns all ~15-20) | Limit result count                                                                                      |
| `--format`      | enum   | text                      | Global option: `text`, `json`, `markdown`                                                               |

**Output formats:**

- **text** (default): Numbered list with title, URL, and truncated snippet (≤200 chars)
- **json**: Full SearXNG response including `results`, `answers`, `suggestions`, `corrections`, `infoboxes`, `unresponsive_engines`
- **markdown**: `### N. [Title](URL)` with snippet

**Examples:**

```bash
# Basic search (text output, ~15-20 results)
searxng-cli search "how to parse JSON in Python"

# Limit results (RECOMMENDED for agents)
searxng-cli search "fastapi middleware" --max-results 5

# News from the past week
searxng-cli search "AI regulation" --category news --time-range week --max-results 5

# IT/developer-focused results
searxng-cli search "tokio runtime" --category it --max-results 5

# JSON for programmatic parsing
searxng-cli --format json search "rust error handling" --max-results 3

# Paginate through results
searxng-cli search "machine learning" --page 2 --max-results 5
```

**JSON response shape:**

```json
{
  "results": [
    {
      "title": "...",
      "url": "...",
      "content": "snippet text",
      "engine": "primary_engine",
      "engines": ["google", "duckduckgo"],
      "score": 1.67,
      "publishedDate": "2024-01-01" | null,
      "category": "general"
    }
  ],
  "answers": [{"answer": "...", "url": "..."}],
  "suggestions": ["related query 1"],
  "corrections": ["did you mean..."],
  "infoboxes": [...],
  "unresponsive_engines": [["engine_name", "reason"]],
  "query": "original query",
  "number_of_results": 0
}
```

### fetch — Extract Page Content

Fetches a URL and extracts content as markdown. Automatically converts HTML to markdown.

```bash
searxng-cli fetch <URL> [OPTIONS]
```

| Option        | Type  | Default | Description                                    |
| ------------- | ----- | ------- | ---------------------------------------------- |
| `--max-chars` | usize | 50,000  | Maximum content length                         |
| `--timeout`   | u64   | 30      | Request timeout in seconds                     |
| `--render`    | flag  | off     | Use headless browser (requires server running) |

**Output** (always JSON regardless of `--format`):

```json
{
  "url": "https://example.com",
  "title": "Page Title",
  "content": "# Heading\n\nMarkdown content...",
  "format": "Markdown",
  "status_code": 200,
  "content_length": 1234
}
```

**Examples:**

```bash
# Simple fetch (lightweight HTTP, HTML→Markdown)
searxng-cli fetch "https://docs.python.org/3/tutorial/index.html"

# Limit content size for token budget
searxng-cli fetch "https://en.wikipedia.org/wiki/Rust_(programming_language)" --max-chars 5000

# JS-heavy pages (requires `searxng-cli serve` running)
searxng-cli fetch "https://react.dev/learn" --render --max-chars 10000

# Longer timeout for slow pages
searxng-cli fetch "https://slow-api.example.com/docs" --timeout 60
```

**Hybrid fetch behavior (with `--render`):**

1. First attempts lightweight HTTP fetch
2. If extracted content < 100 characters → falls back to browser rendering
3. Browser render creates temporary session, navigates, snapshots, then auto-kills the session

**Without `--render`:** Pure HTTP GET → HTML-to-Markdown conversion. Fast, no server needed.

### serve — Start Browser Automation Server

Starts the axum HTTP server with Playwright/CDP browser pool.

```bash
searxng-cli serve [OPTIONS]
```

| Option          | Type | Default | Description |
| --------------- | ---- | ------- | ----------- |
| `--server-port` | u16  | 18960   | Listen port |

**Server capabilities:**

- Browser session pool (max 8 concurrent sessions)
- Session idle timeout: 600 seconds (auto-cleanup)
- Health check endpoint: `GET /api/health`
- Search API: `GET /api/search?query=...`
- Fetch API: `POST /api/fetch`

**Running in background:**

```bash
# tmux (recommended for agents)
tmux new-session -d -s searxng 'searxng-cli serve'

# Verify server is ready
curl -s http://localhost:18960/api/health | jq .data.healthy

# When done
tmux kill-session -t searxng
```

### Browser Commands (require server)

All browser commands communicate with `searxng-cli serve` via HTTP. The `--session` flag is **required** for all browser commands.

#### navigate — Open URL in Browser Session

```bash
searxng-cli navigate --session <ID> <URL>
```

Creates a new session if `<ID>` doesn't exist, or navigates the existing session. Sessions persist until killed or idle-timeout (600s).

**Output:** `Navigation successful` (stdout) or error (stderr, exit code 1)

#### snapshot — Get Page Content

```bash
searxng-cli snapshot --session <ID>
```

Returns raw HTML of the current page. Use for reading page content after navigation/interaction.

**Output:** Raw page HTML to stdout.

#### click — Click an Element

```bash
searxng-cli click --session <ID> <SELECTOR>
```

Clicks element matching CSS selector.

**Output:** `Click successful` or error.

#### fill — Fill Form Field

```bash
searxng-cli fill --session <ID> <SELECTOR> <VALUE>
```

Fills input element matching CSS selector with the given value.

**Output:** `Fill successful` or error.

#### evaluate — Run JavaScript

```bash
searxng-cli evaluate --session <ID> <JS_EXPRESSION>
```

Evaluates JavaScript in page context and returns the result as JSON.

**Output:** JSON value of the expression result.

#### screenshot — Capture Screenshot

```bash
searxng-cli screenshot --session <ID> [--file <PATH>] [--selector <SELECTOR>]
```

Takes PNG screenshot. If `--file` is provided, saves to disk. Otherwise outputs base64-encoded PNG to stdout.

#### tabs — Manage Browser Tabs

```bash
searxng-cli tabs --session <ID> [--action <ACTION>] [--url <URL>]
```

| Action   | Description                            |
| -------- | -------------------------------------- |
| `list`   | List all tabs in session               |
| `new`    | Open new tab (optionally with `--url`) |
| `close`  | Close current tab                      |
| `select` | Switch to a different tab              |

**Output:** JSON response.

#### instances — List Active Sessions

```bash
searxng-cli instances
```

Returns JSON array of all active browser sessions with their history.

**Output:**

```json
{
  "success": true,
  "data": [
    {
      "id": "my-session",
      "tab_count": 1,
      "active_tab": 0,
      "created_at": "2026-07-23T03:40:22Z",
      "history": [{ "command": "navigate", "detail": "https://example.com", "duration_ms": 202, "success": true }]
    }
  ]
}
```

#### kill — Terminate Browser Session

```bash
searxng-cli kill --session <ID>
```

Destroys the browser session and frees resources.

**Output:** `Session <ID> killed` or error if session doesn't exist.

#### session-info — Get Session Details

```bash
searxng-cli session-info --session <ID>
```

Returns detailed JSON info about a specific session including command history.

## Configuration

**Precedence** (lowest → highest): Hard-coded defaults → YAML file → Environment variables → CLI flags

### Config File

Location: `~/.config/searxng-cli/config.yaml`

```yaml
# SearXNG instance URL
searxng_url: 'http://localhost:8888'

# Browser server port
server_port: 18960

# Chromium binary path (null = auto-detect)
chrome_path: null

# URL the CLI uses to reach the browser server
browser_server_url: 'http://localhost:18960'

# Max concurrent browser sessions
max_sessions: 8

# Session auto-kill after idle (seconds)
session_idle_timeout_secs: 600

# HTTP retry configuration
retry:
  max_retries: 3 # Retries on 5xx / connection / timeout errors
  base_delay_ms: 200 # Exponential backoff base (200ms, 400ms, 800ms...)
  timeout_secs: 15 # Per-request timeout

# Search result cache
cache:
  search_ttl_secs: 300 # Cache TTL (5 minutes)
  max_entries: 200 # Max cached queries
```

### Environment Variables

| Variable                     | Description                    | Default                  |
| ---------------------------- | ------------------------------ | ------------------------ |
| `SEARXNG_URL`                | SearXNG instance URL           | `http://localhost:8888`  |
| `SEARXNG_SERVER_PORT`        | Server listen port             | `18960`                  |
| `SEARXNG_CHROME_PATH`        | Path to Chromium/Chrome binary | auto-detect              |
| `SEARXNG_BROWSER_SERVER_URL` | URL CLI uses to reach server   | `http://localhost:18960` |
| `RUST_LOG`                   | Log level for debugging        | none                     |

## Error Handling

### Exit Codes

| Code | Category | Meaning                                          |
| ---- | -------- | ------------------------------------------------ |
| 0    | Success  | Command completed                                |
| 1    | Client   | Bad input, IO error, invalid URL, config error   |
| 2    | Server   | SearXNG error, browser error, server not running |
| 3    | Timeout  | HTTP request timed out                           |
| 4    | Session  | Session not found, session ID required           |

### Error Messages (stderr)

```
Error: Http(reqwest::Error { ... ConnectionRefused ... })     → SearXNG/server not reachable
Error: Browser("Server error: 404 Not Found")                 → Session doesn't exist
Error: Session 'xyz' not found                                → Wrong session ID
```

### Retry Behavior

The CLI automatically retries on:

- HTTP 5xx server errors
- Connection failures (`ECONNREFUSED`, etc.)
- Timeout errors

Does NOT retry on:

- HTTP 4xx client errors (bad request, not found)
- DNS resolution failures
- Redirect errors

Backoff: exponential with jitter (200ms base × 2^attempt + random 0-200ms)

## Best Practices for LLM Agents

### DO ✅

1. **Always use `--max-results 3-5`** for search — without it, returns ~15-20 results eating token budget
2. **Use `--format json`** when you need structured data (URLs, scores, engines)
3. **Use `--format text`** (default) for human-readable quick scans
4. **Use `--max-chars`** on fetch to control content length for token budgets
5. **Kill sessions when done** — sessions consume memory and Chromium processes
6. **Check exit codes** — non-zero means failure, parse stderr for details
7. **Use meaningful session IDs** — `research-session`, `form-fill-1` not `abc123`
8. **Start server in tmux** for persistent background operation
9. **Use `--category`** to narrow search scope — `news` for current events, `it` for tech docs
10. **Use `--time-range`** for freshness-sensitive queries

### DON'T ❌

1. **Don't skip `--max-results`** — unbounded search wastes tokens on 15+ results
2. **Don't use `--render` without server running** — will fail with connection error
3. **Don't forget to kill sessions** — max 8 concurrent, idle timeout is 10 minutes
4. **Don't use browser commands without starting `searxng-cli serve` first**
5. **Don't assume `--format` applies to `fetch`** — fetch always outputs JSON
6. **Don't retry on exit code 1 (client errors)** — bad input won't fix itself
7. **Don't run multiple `searxng-cli serve` instances** — port conflict
8. **Don't use very long session IDs** — they appear in URLs and logs
9. **Don't parse text format programmatically** — use `--format json` instead
10. **Don't expect `number_of_results` to be accurate** — SearXNG often returns 0 here

### Search Strategy (recommended workflow)

```bash
# 1. Broad search to find relevant URLs
searxng-cli search "topic" --max-results 5

# 2. Fetch the most relevant URL(s) for full content
searxng-cli fetch "https://best-result-url.com" --max-chars 10000

# 3. If page is JS-heavy and fetch returns minimal content, use render
searxng-cli fetch "https://spa-app.com/docs" --render --max-chars 10000
```

### Browser Automation (recommended workflow)

```bash
# 1. Ensure server is running
tmux new-session -d -s searxng 'searxng-cli serve'

# 2. Create session and navigate
searxng-cli navigate --session my-task "https://target-site.com"

# 3. Read page content
searxng-cli snapshot --session my-task

# 4. Interact (click, fill, etc.)
searxng-cli click --session my-task "#login-button"
searxng-cli fill --session my-task "#email" "user@example.com"
searxng-cli fill --session my-task "#password" "secret"
searxng-cli click --session my-task "button[type=submit]"

# 5. Verify result
searxng-cli snapshot --session my-task

# 6. Clean up
searxng-cli kill --session my-task

# 7. When done with all browser work
tmux kill-session -t searxng
```

### Token Budget Management

| Scenario                  | Recommended Settings                                                  |
| ------------------------- | --------------------------------------------------------------------- |
| Quick fact lookup         | `search --max-results 3`                                              |
| Research multiple sources | `search --max-results 5` then `fetch --max-chars 5000`                |
| Full article read         | `fetch --max-chars 20000`                                             |
| Code documentation        | `search --category it --max-results 5` then `fetch --max-chars 10000` |
| News briefing             | `search --category news --time-range day --max-results 5`             |

### Common Failure Modes and Recovery

| Error                             | Cause                       | Fix                                                         |
| --------------------------------- | --------------------------- | ----------------------------------------------------------- |
| Connection refused on search      | SearXNG not running         | Start: `docker compose -f docker-compose.dev.yaml up -d`    |
| Connection refused on browser cmd | Server not running          | Start: `searxng-cli serve` (or via tmux)                    |
| Session not found (exit 1)        | Wrong ID or session expired | Use `instances` to list active sessions                     |
| Timeout (exit 3)                  | Slow page or network        | Retry with `--timeout 60`                                   |
| Empty search results              | Bad query or engines down   | Check `unresponsive_engines` in JSON output; rephrase query |
| Fetch returns minimal content     | JS-rendered page            | Retry with `--render` flag                                  |
| Max sessions reached (8)          | Too many open sessions      | Kill unused sessions: `searxng-cli kill --session <id>`     |

## Known Limitations

1. **`--max-results` is advisory** — SearXNG may return more or fewer results depending on engines
2. **`--format` is a GLOBAL option** — place it before or after subcommand, both work: `searxng-cli --format json search "q"` OR `searxng-cli search "q" --format json`
3. **`fetch` ignores `--format`** — always outputs JSON
4. **Search caching** — identical queries within 5 minutes return cached results (no network call)
5. **`snapshot` returns raw HTML** — not a structured accessibility tree despite the name
6. **No built-in pagination for fetch** — use `--max-chars` to control output size
7. **`number_of_results` in search response** — often 0, don't rely on it for counting
8. **Session auto-expiry** — sessions die after 10 minutes of inactivity without warning
9. **`unresponsive_engines`** — some engines (brave, qwant, wikidata) frequently get rate-limited/suspended
