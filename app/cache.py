"""SQLite content-addressed cache for refined Markdown and image captions.

Two tables:
- ``page_cache``: keyed by hash of the markdownify Draft.  Stores the
  Refiner's marker'd output, a caption map, and the set of missing image
  indices.
- ``image_cache``: keyed by hash of image bytes.  Stores the Captioner's
  caption.

Connection per request, WAL mode for concurrent reads.  No TTL.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import sqlite3
from contextlib import asynccontextmanager
from dataclasses import dataclass
from datetime import datetime, UTC
from pathlib import Path
from typing import Any


def _normalize_db_path(db_path: str) -> str:
    """Normalize :memory: to a temp file for cross-connection persistence."""
    if db_path == ":memory:":
        import tempfile
        fd, path = tempfile.mkstemp(suffix=".db", prefix="mcp_searxng_")
        import os
        os.close(fd)
        os.unlink(path)
        return path
    return db_path


def hash_draft(draft: str) -> str:
    """SHA-256 hash of the markdownify Draft.

    Args:
        draft: The markdownify Draft string.

    Returns:
        Hex digest string (64 chars).
    """
    return hashlib.sha256(draft.encode("utf-8")).hexdigest()


def hash_image_bytes(image_bytes: bytes) -> str:
    """SHA-256 hash of image bytes.

    Args:
        image_bytes: Raw image bytes.

    Returns:
        Hex digest string (64 chars).
    """
    return hashlib.sha256(image_bytes).hexdigest()


def splice_captions(
    refiner_output: str,
    captions: dict[int, str],
    image_map: dict[int, dict[str, str]],
) -> str:
    """Replace ``<!--IMG:i-->`` markers with ``![caption](url)`` syntax.

    Args:
        refiner_output: The Refiner's marker'd output (clean ``<!--IMG:i-->`` markers).
        captions: Map of ``{image_index: caption_text}`` (only available captions).
        image_map: Map of ``{image_index: {url, alt}}`` from the pre-pass.

    Returns:
        Markdown with captions spliced in as ``![caption](url)``.  Missing
        captions produce ``![](url)``.
    """
    import re

    pattern = re.compile(r"<!--IMG:(\d+)-->")

    def _replace(match: re.Match) -> str:
        index = int(match.group(1))
        url = image_map.get(index, {}).get("url", "")
        caption = captions.get(index, "")
        # Escape the caption for markdown alt text (brackets)
        safe_caption = caption.replace("[", "\\[").replace("]", "\\]")
        return f"![{safe_caption}]({url})"

    return pattern.sub(_replace, refiner_output)


@dataclass(frozen=True, slots=True)
class PageCacheEntry:
    """A cached page refinement result.

    Attributes:
        hash: SHA-256 of the Draft.
        refiner_output: Refiner's marker'd output (with ``<!--IMG:i-->`` markers).
        captions: Map of ``{image_index: caption_text}``.
        missing: List of image indices still missing captions.
        created_at: ISO timestamp.
    """

    hash: str
    refiner_output: str
    captions: dict[int, str]
    missing: list[int]
    created_at: str


class PageCache:
    """SQLite-backed page cache, keyed by Draft hash."""

    def __init__(self, db_path: str) -> None:
        """Initialize the page cache.

        Args:
            db_path: Path to the SQLite database file (or ``:memory:``).
        """
        self._db_path = _normalize_db_path(db_path)

    async def init(self) -> None:
        """Create the page_cache table if it doesn't exist."""
        await self._execute("""
            CREATE TABLE IF NOT EXISTS page_cache (
                hash TEXT PRIMARY KEY,
                refiner_output TEXT NOT NULL,
                captions TEXT NOT NULL DEFAULT '{}',
                missing TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL
            )
        """)

    async def get(self, hash: str) -> PageCacheEntry | None:
        """Look up a page cache entry by hash.

        Args:
            hash: SHA-256 of the Draft.

        Returns:
            A :class:`PageCacheEntry` or ``None`` on miss.
        """
        row = await self._fetchone(
            "SELECT refiner_output, captions, missing, created_at FROM page_cache WHERE hash = ?",
            (hash,),
        )
        if row is None:
            return None
        refiner_output, captions_json, missing_json, created_at = row
        captions: dict[int, str] = {
            int(k): v for k, v in json.loads(captions_json).items()
        }
        missing: list[int] = json.loads(missing_json)
        return PageCacheEntry(
            hash=hash,
            refiner_output=refiner_output,
            captions=captions,
            missing=missing,
            created_at=created_at,
        )

    async def store(
        self,
        hash: str,
        refiner_output: str,
        captions: dict[int, str],
        missing: list[int],
    ) -> None:
        """Store a page cache entry (INSERT OR REPLACE).

        Args:
            hash: SHA-256 of the Draft.
            refiner_output: Refiner's marker'd output.
            captions: Map of available captions.
            missing: List of image indices still missing.
        """
        now = datetime.now(UTC).isoformat()
        captions_json = json.dumps({str(k): v for k, v in captions.items()})
        missing_json = json.dumps(missing)
        await self._execute(
            """INSERT OR REPLACE INTO page_cache
               (hash, refiner_output, captions, missing, created_at)
               VALUES (?, ?, ?, ?, ?)""",
            (hash, refiner_output, captions_json, missing_json, now),
        )

    async def update_captions(
        self,
        hash: str,
        captions: dict[int, str],
        missing: list[int],
    ) -> None:
        """Update the caption map and missing list for an existing entry.

        Args:
            hash: SHA-256 of the Draft.
            captions: Updated map of available captions.
            missing: Updated list of missing image indices.
        """
        captions_json = json.dumps({str(k): v for k, v in captions.items()})
        missing_json = json.dumps(missing)
        await self._execute(
            """UPDATE page_cache SET captions = ?, missing = ? WHERE hash = ?""",
            (captions_json, missing_json, hash),
        )

    async def _execute(self, sql: str, params: tuple = ()) -> None:
        """Execute a SQL statement (no return)."""
        await asyncio.to_thread(self._execute_sync, sql, params)

    def _execute_sync(self, sql: str, params: tuple) -> None:
        conn = sqlite3.connect(self._db_path)
        try:
            conn.execute("PRAGMA journal_mode=WAL")
            conn.execute(sql, params)
            conn.commit()
        finally:
            conn.close()

    async def _fetchone(self, sql: str, params: tuple = ()) -> Any:
        """Execute a SELECT and return one row (or None)."""
        return await asyncio.to_thread(self._fetchone_sync, sql, params)

    def _fetchone_sync(self, sql: str, params: tuple) -> Any:
        conn = sqlite3.connect(self._db_path)
        try:
            conn.execute("PRAGMA journal_mode=WAL")
            cursor = conn.execute(sql, params)
            row = cursor.fetchone()
            return row
        finally:
            conn.close()


class ImageCache:
    """SQLite-backed image cache, keyed by image byte hash."""

    def __init__(self, db_path: str) -> None:
        """Initialize the image cache.

        Args:
            db_path: Path to the SQLite database file (or ``:memory:``).
        """
        self._db_path = _normalize_db_path(db_path)

    async def init(self) -> None:
        """Create the image_cache table if it doesn't exist."""
        await self._execute("""
            CREATE TABLE IF NOT EXISTS image_cache (
                image_hash TEXT PRIMARY KEY,
                caption TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
        """)

    async def get(self, image_hash: str) -> str | None:
        """Look up a cached caption by image hash.

        Args:
            image_hash: SHA-256 of the image bytes.

        Returns:
            The cached caption, or ``None`` on miss.
        """
        row = await self._fetchone(
            "SELECT caption FROM image_cache WHERE image_hash = ?",
            (image_hash,),
        )
        if row is None:
            return None
        return row[0]

    async def store(self, image_hash: str, caption: str) -> None:
        """Store a caption (INSERT OR REPLACE).

        Args:
            image_hash: SHA-256 of the image bytes.
            caption: The Captioner's output.
        """
        now = datetime.now(UTC).isoformat()
        await self._execute(
            """INSERT OR REPLACE INTO image_cache
               (image_hash, caption, created_at)
               VALUES (?, ?, ?)""",
            (image_hash, caption, now),
        )

    async def _execute(self, sql: str, params: tuple = ()) -> None:
        """Execute a SQL statement (no return)."""
        await asyncio.to_thread(self._execute_sync, sql, params)

    def _execute_sync(self, sql: str, params: tuple) -> None:
        conn = sqlite3.connect(self._db_path)
        try:
            conn.execute("PRAGMA journal_mode=WAL")
            conn.execute(sql, params)
            conn.commit()
        finally:
            conn.close()

    async def _fetchone(self, sql: str, params: tuple = ()) -> Any:
        """Execute a SELECT and return one row (or None)."""
        return await asyncio.to_thread(self._fetchone_sync, sql, params)

    def _fetchone_sync(self, sql: str, params: tuple) -> Any:
        conn = sqlite3.connect(self._db_path)
        try:
            conn.execute("PRAGMA journal_mode=WAL")
            cursor = conn.execute(sql, params)
            row = cursor.fetchone()
            return row
        finally:
            conn.close()
