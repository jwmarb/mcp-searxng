# mcp-searxng

An MCP server that exposes SearXNG meta-search and page-reading tools to AI
agents, with content-addressed caching of refined Markdown and image captions.

## Language

### Tools

**web_search**:
MCP tool. Takes a query, delegates to SearXNG, returns formatted text.
Unchanged from the original implementation. Not cached, no LLM involved.
_Avoid_: search.

**web_url_read**:
MCP tool. Takes a URL, runs the full pipeline (Primary fetch → Fallback
fetch on failure → markdownify → hash → Page-cache lookup → Refiner +
Captioner in parallel → splice → store → return). Returns Markdown on
success, a human-readable error string on failure. String in, string out.
_Avoid_: read_url, fetch_url.

**web_url_read_from_html**:
MCP tool. Takes raw HTML, runs markdownify → hash → Page-cache lookup →
Refiner + Captioner → splice → store → return. The escape hatch for
clients that fetch the page themselves. Also the internal code path
`web_url_read` uses after the Fallback agent saves HTML to a temp file.
_Avoid_: read_html, parse_html.

### Fetching & rendering

**Page**:
A single URL's worth of HTML content, however it was retrieved.
_Avoid_: article, document, webpage (use Page when talking about the fetch/cache unit).

**Primary fetch**:
Retrieving a Page's HTML via `httpx` (a plain HTTP GET). The fast path; used
unless it fails.
_Avoid_: direct fetch, simple fetch.

**Fallback fetch**:
Retrieving a Page's HTML via a bounded LLM tool-use agent hosted in-process
by this server. The agent connects to `playwright-mcp` (as an MCP client)
and uses its browser tools to reach pages the Primary fetch can't
(Cloudflare challenge, JS-only render). The agent's sole goal: get the
page's main content and save it to a temp HTML file. The agent is
unaware of images. Once the HTML file exists, the normal pipeline
(markdownify -> hash -> cache lookup -> Refiner) takes over. Image
capture is infrastructure, not agent work: the Playwright session is
configured to auto-capture `image/*` responses during render, and after
the agent terminates, the server mechanically fetches any missed images
(lazy-loaded, JS-inserted) via the still-open browser session — no LLM
judgment needed. Invoked only on Primary-fetch failure.

The `playwright-mcp` subprocess is launched per Fallback fetch and
terminated after the fetch completes (and the server's post-agent image
fetches finish). No persistent browser session; each Fallback fetch
gets a clean browser state with no cookie carryover. The cost: every
Fallback fetch pays the full startup cost (subprocess + browser launch),
and repeated Fallback fetches to the same site re-solve challenges from
scratch. The benefit: zero idle resource use and clean state per fetch.
`playwright-mcp` is installed via Docker (Dockerfile installs Node + the
package + browser binaries); local dev requires manual installation
(documented in the README).
_Avoid_: browser fetch, headless fetch, scrape.

**Refiner**:
A text-to-text LLM that cleans a `markdownify` draft into final Markdown.
Its contract is structural cleanup + boilerplate removal: repair broken
tables, fix heading hierarchy, collapse redundant whitespace, remove
leftover HTML artifacts, and excise semantic boilerplate that markdownify
can't catch (nav menus, cookie banners, related-posts sections, comment
sections, footer links, share buttons, ad blocks). Article content is
preserved verbatim — the Refiner does not summarize, compress, rephrase, or
drop content from the article itself. Boilerplate removal is a judgment
call (e.g. an author bio may be kept or removed depending on whether it
reads as article content or chrome). Emits positional image markers
(e.g. `<!--IMG:i-->`) at the positions where article-relevant `<img>`
tags appeared; boilerplate images (nav logos, ad images) are dropped along
with their surrounding boilerplate. Runs on every page-cache miss, in
parallel with the Captioner.
_Avoid_: summarizer, formatter, extractor.

**Captioner**:
A vision-capable LLM that takes image bytes and produces a comprehensive
description of the image's visible contents. Output is detailed, not
summarized: contents, any text within the image (OCR'd verbatim), colors,
layout, spatial relationships, inferred image type (screenshot, photograph,
chart, diagram, etc.). No length cap — information integrity is the
priority; the client LLM (not this server) decides what to attend to.
Runs on every image *the Refiner keeps* on a page-cache miss — even
images that already have author-provided `alt`, which is passed to the
Captioner as context. The Captioner's output replaces the existing `alt`.
Boilerplate images dropped by the Refiner (nav logos, ad images, dropped
along with their surrounding boilerplate text) are never captioned —
no vision call is spent on them. Runs in parallel with the Refiner; the
two are independent. Caption lookup is by positional index assigned in
document order from the Draft, enabling splice-after-parallel.
_Avoid_: image describer, OCR (use Captioner; OCR is one part of its
output, not its whole job).

### LLM configuration

**LLM role**:
One of three distinct purposes an LLM serves in the pipeline: the Refiner
(text-to-text Markdown cleanup), the Captioner (vision, image-to-text),
or the Fallback-fetch agent (vision, tool-use, browser orchestration).
Each role is independently configurable — base URL, API key, and model name
— via `.env`. Power users self-hosting different models for different
roles configure each independently; there is no shared default.
_Avoid_: model, endpoint (use LLM role when talking about the purpose).

**Content-addressed cache**:
A cache whose key is a hash of the content it stores, not the URL it was
fetched from. Two fetches with identical content share one entry.
_Avoid_: URL cache, response cache.

**Draft**:
The output of running `markdownify` on a Page's HTML, before any LLM
refinement. Produced by a custom `markdownify` converter that overrides
image handling: each `<img>` is replaced with a positional marker
`<!--IMG:i url=... alt=...-->` (index in document order, URL and
original alt as context for the Refiner). A side map
`{i: {url, original_alt}}` is produced alongside the Draft. Deterministic,
free, local. The basis for the page cache key. Before splice, the
marker's `url=... alt=...` attributes are stripped, leaving `<!--IMG:i-->`
for the splice step to replace with `![caption_i](url)`.
_Avoid_: raw markdown, extracted text.

**Page cache**:
SQLite database (single file at `SEARXNG_CACHE_PATH`, default `./cache.db`)
keyed by hash of the Draft. Stores the Refiner's marker'd output (with
`<!--IMG:i-->` markers intact), a caption map (`{image_index:
caption_text}`), and the set of missing image indices — not the final
spliced Markdown. The spliced Markdown is rendered from these components
on every read (a cheap string-replacement step). Connection per request,
WAL mode (`PRAGMA journal_mode=WAL`) for concurrent reads + serialized
writes. No TTL. The URL is not part of the key; the fetch must happen
before the lookup, since only the fetched content's hash can answer "is
this content cached?" On a hash hit where the entry is *incomplete*, the
Refiner is skipped (cached marker'd output is reused as the base) and only
the missing image indices are re-captioned; the Image cache is consulted
first, so previously-succeeded images are free. When all images are
captioned, the entry is marked complete and future hits splice all
captions with no LLM calls. Images are supporting context for the text,
not primary content — partial pages are served to the client rather than
blocking on image failures.
_Avoid_: markdown cache, URL cache.

**Image cache**:
SQLite table keyed by hash of the image bytes, storing the Captioner's
caption. No TTL. Same image on different pages is captioned once.
_Avoid_: alt cache.

## Flagged ambiguities

- **"Vision-capable LLM"**: resolved. Means a model that accepts text AND
  image input. Two roles use vision: the Captioner (image bytes -> caption
  text) and the Fallback-fetch agent (screenshots -> page-state judgment for
  Cloudflare/JS-heavy pages). The Refiner is text-only. The term "vision"
  describes a capability, not a role.

- **"LLM agent handles Playwright"**: resolved. A bounded tool-use loop
  hosted in-process by the MCP server. The loop is hand-rolled (no agent
  framework) using the OpenAI-compatible function-calling API, optionally
  via `litellm` for provider normalization. The server acts as a bridge
  between the LLM (function-calling) and `playwright-mcp` (MCP protocol):
  it queries playwright-mcp's tool list, presents those tools to the LLM,
  forwards the LLM's tool calls to playwright-mcp, and returns results.
  The agent connects to `playwright-mcp` as an MCP client via stdio
  (spawned as a subprocess per Fallback fetch, terminated after). The
  agent's only job is to get the page's main content and save it to a temp
  HTML file; once the file exists, the normal pipeline takes over. Hard
  cap on iterations (≤5). Invoked only on Primary-fetch failure.
  OpenAI-compatible LLM endpoint, configured via `.env`. Not a
  general-purpose agent.

## Domain constraints

- **Local-first LLMs.** Users are expected to run LLMs locally (e.g. via an
  OpenAI-compatible local server). Token usage is not a concern. The cache
  exists to avoid redundant work and latency-and-capacity contention for the
  local LLM, not to save token cost. Re-running the Refiner on an identical
  Draft would produce identical output (deterministic local model) while
  blocking the local LLM from other work — so the cache skips it.

- **LLM roles are independently configured.** Each of the three roles
  (Refiner, Captioner, Fallback-fetch agent) has its own `*_BASE_URL`,
  `*_API_KEY`, and `*_MODEL` in `.env`. There is no shared default model
  or endpoint. Power users self-hosting applications are expected to
  configure each role deliberately, choosing the right model for each
  purpose (e.g. a fast text model for the Refiner, a vision model for the
  Captioner, a tool-use-capable model for the agent).

- **Graceful degradation by role.** Each LLM role fails independently and
  gracefully:
  - **Refiner unavailable:** return the unrefined `markdownify` Draft as-is,
    *do not cache it*. The next fetch re-runs markdownify and retries the
    Refiner; when it's available, the refined version is cached. Cache hits
    (from before the outage) still serve refined Markdown normally.
  - **Captioner unavailable:** cache the page as incomplete (all images
    missing), serve the article text with uncaptioned images. Self-heals
    on next fetch when the Captioner returns (per the incomplete-page
    retry design). Images are supporting context, not primary content.
  - **Agent unavailable:** hard-fail that URL with a specific error
    ("Primary fetch failed and Fallback agent is not configured"). No
    fallback exists — there is no HTML to degrade to.
  - **Startup:** the server always starts regardless of LLM configuration.
    `web_search` works without any LLM. Unconfigured endpoints log warnings
    at startup and fail at use time with specific errors.

- **No TTL, no URL→hash layer.** Both the Page cache and Image cache are
  content-addressed (keyed by content hash, not URL). The URL is never part
  of the cache key and is never consulted in the hit/miss decision. The fetch
  always happens first; the cache decides whether the Refiner runs, not
  whether the fetch runs.

## Example dialogue

**Dev**: "I fetched the Page, Primary fetch failed — Cloudflare. Kick the
Fallback-fetch agent, get HTML back, run it through `markdownify` to get the
Draft, hash it. Page-cache hit?"

**Domain expert**: "If the hash matches an existing Page-cache entry, splice
the cached marker'd output with its caption map and return — no Refiner, no
Captioner. If miss, pre-pass the Draft to enumerate `<img>` tags in document
order (custom markdownify emits `<!--IMG:i url=... alt=...-->` markers and a
side map), then run the Refiner and Captioner in parallel: Refiner cleans the
text, drops boilerplate blocks (including their images), and emits markers
only for article-relevant images; Captioner describes each kept image.
Splice captions into the markers, store under the Draft's hash."

**Dev**: "What if an image's bytes are identical to one I've captioned
before on a different page?"

**Domain expert**: "The Image cache is keyed by image-byte hash. The
Captioner checks it before running — identical bytes return the cached
caption without a vision call. Different page, same image, one caption."

**Dev**: "What if the Captioner fails on 2 of 10 images?"

**Domain expert**: "Store the page as incomplete — Refiner output + caption
map with 8 entries + missing list `[8, 9]`. Splice what we have, return to
the client (text is primary, images are supporting context). Next fetch:
Draft hash matches, Page-cache hit but incomplete. Skip the Refiner, retry
only indices 8 and 9 (Image cache first, then Captioner on miss). When all
10 are done, mark the entry complete; future hits splice all captions with
no LLM calls."
