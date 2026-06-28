# Content-addressed cache, no TTL, SQLite

The cache for refined Markdown and image captions is content-addressed: the
Page cache is keyed by a hash of the `markdownify` Draft (not the URL), and
the Image cache is keyed by a hash of the image bytes. Both are stored in a
single SQLite database file at `SEARXNG_CACHE_PATH` (default `./cache.db`).
There is no TTL and no URL→hash layer.

## Considered options

- **URL-keyed cache with TTL (Redis/Valkey).** Key entries by URL, expire
  after a time window. Rejected: the URL is the wrong key — it can't answer
  "has the content changed?" without a fetch, and TTL is a time-based guess
  that's strictly weaker than content-based invalidation. Redis/Valkey adds
  a distributed dependency for a single-user local server.

- **URL→hash index + hash→markdown.** Keep a URL index alongside the
  content-addressed cache to avoid re-fetching unchanged pages. Rejected:
  the fetch can't be skipped (you can't know if a page changed without
  fetching it, modulo HTTP ETags). The URL index is a redundant lookup that
  produces the same hit/miss answer as the content-addressed lookup, one
  step later.

- **Content-addressed, no TTL, SQLite (chosen).** The Draft hash is computed
  on every fetch (markdownify is free and local). The cache decides whether
  the Refiner runs, not whether the fetch runs. SQLite with WAL mode handles
  concurrent reads. Storage grows monotonically but is bounded by distinct
  page contents, which is small for personal use.

## Consequences

- The URL is never part of the cache key and is never consulted in the
  hit/miss decision. The fetch always happens first.
- Storage is monotonic (no eviction). For personal use, the realistic
  ceiling is well under SQLite's capacity. A manual maintenance tool (clear
  by age or hash) can be added if storage ever becomes a concern.
- HTTP-level caching (ETag / If-None-Match) is a separate future
  optimization that would avoid re-downloading unchanged pages. It's
  orthogonal to this design and can be added without changing the cache key.
