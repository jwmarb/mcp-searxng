"""MCP tool implementations for SearXNG."""

from urllib.parse import quote

from app import mcp
from app.client import read_url, search


@mcp.tool()
async def web_search(query: str, count: int = 3) -> str:
    """Performs a web search using the SearxNG API.

    Ideal for general queries, news, articles, and online content.
    Use this for broad information gathering, recent events,
    or when you need diverse web sources.
    """
    return await search(query, limit=count)


@mcp.tool()
async def web_url_read(url: str) -> str:
    """Read the content from a URL and return it as Markdown.

    Use this for retrieving and understanding the content of a specific webpage.
    """
    return await read_url(url)