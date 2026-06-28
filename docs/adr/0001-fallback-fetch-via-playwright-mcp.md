# Fallback fetch via playwright-mcp

When the Primary fetch (httpx) fails — Cloudflare challenge, JS-only render
— the Fallback fetch is a bounded LLM tool-use agent hosted in-process by
this MCP server. The agent connects to `playwright-mcp` as an MCP client via
stdio (spawning playwright-mcp as a subprocess) and uses its browser tools to
reach the page, extract the HTML, and save it to a temp file. The normal
pipeline (markdownify -> hash -> cache -> Refiner) then takes over.

## Considered options

- **playwright-python directly.** Agent calls `playwright-python` bindings
  in-process, no MCP intermediary. Rejected: reinvents the tool surface
  playwright-mcp already provides (navigate, click, screenshot, wait, etc.),
  and couples this server to Playwright's API directly.

- **playwright-mcp as a subprocess (chosen).** Server spawns playwright-mcp
  via stdio and connects as an MCP client. The agent uses playwright-mcp's
  tools. Self-contained: the user configures one MCP server (this one); this
  server manages its playwright-mcp subprocess internally.

- **Push orchestration to the upstream MCP client.** This server exposes
  `web_url_read` (httpx) and `web_url_read_from_html` (HTML input) only; the
  upstream client (Claude, opencode, etc.) orchestrates playwright-mcp and
  calls the second tool on failure. Rejected: reverses the Q2 decision to
  host the agent in-process, and pushes orchestration burden to every MCP
  client that uses this server — more setup, more fragile.

## Consequences

- This MCP server is both an MCP server (to its upstream clients) and an MCP
  client (to playwright-mcp). That dual role is unusual but valid.
- Runtime gains a dependency on playwright-mcp being installed and on an
  OpenAI-compatible LLM endpoint (configured via `.env`).
- The user's MCP config stays simple: one server to configure. This server
  spawns playwright-mcp internally; the user does not configure playwright-mcp
  separately.
