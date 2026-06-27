"""SearXNG API client."""

import logging
from os import getenv

from httpx import AsyncClient

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
    client = AsyncClient(base_url=str(getenv("SEARXNG_URL", "http://localhost:8080")))

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


async def read_url(url: str) -> str:
    """Fetch a URL and convert its HTML content to Markdown.

    Args:
        url: The URL to fetch.

    Returns:
        Markdown-formatted content of the page, or an error message string.
    """
    from markdownify import markdownify as md
    from httpx import HTTPStatusError

    headers = {"User-Agent": "MCP-SEARXNG"}

    try:
        async with AsyncClient(
            follow_redirects=True,
            headers=headers,
            timeout=10.0,
            max_redirects=5,
        ) as client:
            response = await client.get(url)
            response.raise_for_status()
            return md(response.text)
    except HTTPStatusError as e:
        logger.error("HTTP error fetching URL %s: %s", url, e)
        return f"Error fetching URL {url}: {e}"
    except Exception as e:
        logger.error("Unexpected error fetching URL %s: %s", url, e)
        return f"Error fetching URL {url}: {e}"