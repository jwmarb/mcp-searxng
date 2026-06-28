"""SearXNG API client and page fetching with Primary/Fallback paths."""

from __future__ import annotations

import asyncio
import logging
from os import getenv
from urllib.parse import urljoin, urlparse

import httpx
from httpx import HTTPStatusError

from app.agent import AgentError, fallback_fetch_html
from app.config import get_settings
from app.models import Response

logger = logging.getLogger(__name__)


async def search(query: str, limit: int = 3) -> str:
    """Search SearXNG and format results as plain text.

    Args:
        query: The search query string.
        limit: Maximum number of results to include (default 3).

    Returns:
        Formatted text containing infoboxes and search results.
    """
    settings = get_settings()
    async with httpx.AsyncClient(base_url=settings.searxng_url) as client:
        params: dict[str, str] = {"q": query, "format": "json"}
        response = await client.get("/search", params=params)
        response.raise_for_status()

        data = Response.model_validate_json(response.text)

        text = ""
        for infobox in data.infoboxes:
            text += f"Infobox: {infobox.infobox}\n"
            text += f"ID: {infobox.id}\n"
            text += f"Content: {infobox.content}\n"
            text += "\n"

        if not data.results:
            text += "No results found\n"

        for index, result in enumerate(data.results):
            text += f"Title: {result.title}\n"
            text += f"URL: {result.url}\n"
            text += f"Content: {result.content}\n"
            text += "\n"
            if index == limit - 1:
                break

        return text


async def fetch_html(url: str) -> str:
    """Fetch HTML from a URL, trying Primary (httpx) then Fallback (agent).

    Args:
        url: The URL to fetch.

    Returns:
        The page's HTML content.

    Raises:
        Exception: If both Primary and Fallback fetches fail.
    """
    html = await _primary_fetch(url)
    if html is not None:
        return html

    logger.info("Primary fetch failed for %s, trying Fallback agent", url)
    html, _images = await fallback_fetch_html(url)
    return html


async def _primary_fetch(url: str) -> str | None:
    """Primary fetch via httpx.

    Returns:
        HTML string on success, None on failure.
    """
    headers = {"User-Agent": "MCP-SEARXNG"}
    try:
        async with httpx.AsyncClient(
            follow_redirects=True,
            headers=headers,
            timeout=10.0,
            max_redirects=5,
        ) as client:
            response = await client.get(url)
            response.raise_for_status()
            return response.text
    except HTTPStatusError as exc:
        logger.error("HTTP error fetching %s: %s", url, exc)
        return None
    except Exception as exc:
        logger.error("Primary fetch error for %s: %s", url, exc)
        return None


async def fetch_image_bytes(
    image_url: str, base_url: str | None = None
) -> bytes | None:
    """Fetch image bytes from a URL, resolving relative URLs against base_url.

    Args:
        image_url: The image URL (may be relative).
        base_url: Base URL for resolving relative URLs.

    Returns:
        Image bytes on success, None on failure.
    """
    absolute_url = _resolve_url(image_url, base_url)
    headers = {"User-Agent": "MCP-SEARXNG"}
    try:
        async with httpx.AsyncClient(
            follow_redirects=True,
            headers=headers,
            timeout=15.0,
        ) as client:
            response = await client.get(absolute_url)
            response.raise_for_status()
            return response.content
    except Exception as exc:
        logger.warning("Failed to fetch image %s: %s", absolute_url, exc)
        return None


def _resolve_url(url: str, base_url: str | None = None) -> str:
    """Resolve a possibly-relative URL against a base URL.

    Args:
        url: The URL to resolve (may be relative).
        base_url: The base URL for resolution.

    Returns:
        An absolute URL string.
    """
    if url.startswith(("http://", "https://", "data:")):
        return url
    if base_url:
        return urljoin(base_url, url)
    return url
