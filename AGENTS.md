# PROJECT KNOWLEDGE BASE

**Generated:** 2026-07-22
**Commit:** 156d392
**Branch:** main

## OVERVIEW
Rust CLI + axum server for SearXNG search, web fetching, and Playwright browser automation. Designed for LLM agents needing internet access. Dual-language: Rust (`searxng-cli` binary) + Python (MCP server container).

## STRUCTURE
```
.
├── src/
│   ├── main.rs           # CLI entry; command dispatch (13 subcommands)
│   ├── lib.rs            # Public module re-exports
│   ├── cli.rs            # clap derive subcommands + args structs
│   ├── config.rs         # XDG config + env var precedence + CacheConfig
│   ├── error.rs          # CliError enum + ApiError + axum IntoResponse
│   ├── response.rs       # JSON envelope (ApiResponse<T>)
│   ├── time.rs           # Shared ISO-8601 formatter (no-alloc, no chrono)
│   ├── retry.rs          # RetryClient: exponential backoff + jitter
│   ├── browser_client.rs # HTTP client → local /api/* server (CLI-side)
│   ├── browser/          # BrowserManager (playwright-cdp) + mpsc Pool
│   ├── fetch/            # Lightweight HTTP + hybrid JS fallback
│   ├── search/           # SearXNG JSON API client + formatters
│   └── server/           # axum router, SessionManager, history middleware
├── tests/                # Integration tests (wiremock, 6 files)
├── searxng-docker/       # SearXNG settings.yml + uwsgi.ini
├── Dockerfile            # Python MCP server image (bundles Node+Playwright)
├── docker-compose.yaml   # Full stack: MCP + SearXNG on shared network
└── docker-compose.dev.yaml  # SearXNG only (port 8888)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Add CLI subcommand | `cli.rs` + match arm in `main.rs` | clap derive macros |
| Add /api/* endpoint | `server/routes.rs` | axum Extension<SessionManager/Search/Config> |
| Browser session lifecycle | `browser/pool.rs` | mpsc channel + dedicated std::thread |
| Search SearXNG | `search/mod.rs` | reqwest → JSON; moka cache layer |
| Hybrid fetch (HTTP→browser) | `fetch/hybrid.rs` | JS_HEAVY_THRESHOLD = 100 chars |
| Config precedence | `config.rs` | defaults → YAML → env → CLI flags |
| Error types | `error.rs` | thiserror; ErrorCode → exit codes (0-4) |
| Session history tracking | `server/history.rs` | with_history() middleware pattern |
| HTTP retry logic | `retry.rs` | Retries 5xx + connect/timeout; NOT 4xx |
| Timestamps | `time.rs` | format_timestamp(), iso_timestamp() |
| CLI→server bridge | `browser_client.rs` | Thin HTTP client for browser commands |

## CODE MAP

| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `Cli` | struct | `cli.rs` | clap root; global flags + subcommands |
| `Config` | struct | `config.rs` | XDG YAML + env merge; CacheConfig + RetryConfig |
| `CliError` | enum | `error.rs` | Unified error; IntoResponse impl |
| `ApiError` | struct | `error.rs` | JSON error response for axum routes |
| `Search` | struct | `search/mod.rs` | SearXNG HTTP client + moka cache |
| `Fetcher` | struct | `fetch/mod.rs` | HTTP + hybrid fetch dispatcher |
| `RetryClient` | struct | `retry.rs` | reqwest wrapper with exponential backoff |
| `BrowserManager` | struct | `browser/mod.rs` | Playwright launch/shutdown |
| `BrowserPoolHandle` | struct | `browser/pool.rs` | mpsc session pool (dedicated thread) |
| `SessionManager` | struct | `server/session.rs` | Routes → pool + history recording |
| `BrowserClient` | struct | `browser_client.rs` | CLI → local server HTTP client |
| `create_router` | fn | `server/routes.rs` | axum Router builder (12 endpoints) |
| `create_api_router` | fn | `server/routes.rs` | Subset: health + search + fetch only |
| `with_history` | fn | `server/history.rs` | Async middleware: timing + history |
| `hybrid_fetch` | fn | `fetch/hybrid.rs` | Lightweight→browser fallback |
| `format_timestamp` | fn | `time.rs` | Duration → ISO-8601 string (no-alloc) |

## CONVENTIONS
- Edition 2021. Zero `unsafe` — uses compile-time `Send+Sync` assertions instead.
- `Result<T> = std::result::Result<T, CliError>` defined in `error.rs`.
- ISO-8601 timestamps: hand-rolled in `time.rs` (no chrono dep). All modules import from there.
- Tests: inline `#[cfg(test)]` modules in every source file. Integration tests in `tests/` (wiremock).
- Browser pool runs on a dedicated `std::thread` + current_thread tokio runtime (avoids blocking main tokio).
- Config: XDG `~/.config/searxng-cli/config.yaml`. Env vars: `SEARXNG_URL`, `SEARXNG_SERVER_PORT`, `SEARXNG_CHROME_PATH`, `SEARXNG_BROWSER_SERVER_URL`.
- No `#[allow(...)]`, `#[deny(...)]`, or lint attributes anywhere.
- No rustfmt.toml, clippy.toml, or .editorconfig — uses Rust defaults.
- reqwest uses rustls-tls (no openssl dep).

## ANTI-PATTERNS (THIS PROJECT)
- No CI/CD pipeline (no `.github/workflows/`).
- No Makefile/justfile — raw `cargo`/`uv`/`docker compose` commands only.
- No MSRV pin (`rust-version` not set in Cargo.toml).
- No pinned Playwright version in Dockerfile (`@playwright/mcp@latest`).
- `.cargo/config.toml` has musl static-linking target defined but commented out.
- 14 `.unwrap()` calls in production route handlers (`server/routes.rs`).

## COMMANDS
```bash
# Env
conda activate mcp-searxng

# Rust CLI
cargo build                   # debug
cargo build --release         # opt-level z, LTO, stripped
cargo test                    # unit + integration

# Python MCP server (Dockerfile entrypoint)
uv sync && uv run -m app

# Docker
docker compose -f docker-compose.dev.yaml up   # SearXNG on :8888
docker compose up --build                      # full stack: MCP (:5488) + SearXNG
```

## NOTES
- SearXNG dev port: `:8888`. MCP server port: `:18960` (Rust default) / `:5488` (Docker MCP).
- Dockerfile bundles Node.js 22 + `@playwright/mcp` inside Python 3.12-slim-bullseye.
- Release binary: aggressive min-size profile (`opt-level="z"`, `lto=true`, `strip=true`, `panic="abort"`).
- `RetryClient` retries on 5xx + connect/timeout errors only. 4xx, DNS failures, redirects → no retry.
- Browser pool: mpsc channel pattern; `PoolCmd` enum dispatches to `PoolInner` on dedicated thread.
- ~6.5k lines Rust across 27 .rs files. Largest: `browser/pool.rs` (622), `server/routes.rs` (499).