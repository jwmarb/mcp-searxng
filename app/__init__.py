"""MCP server for SearXNG meta-search engine."""

from mcp.server.fastmcp import FastMCP

mcp = FastMCP("mcp-searxng")

__all__ = ["mcp"]