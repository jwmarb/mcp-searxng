"""Fallback-fetch agent: bounded tool-use loop driving playwright-mcp.

When the Primary fetch (httpx) fails, this agent is invoked. It spawns
playwright-mcp as a subprocess via stdio, connects as an MCP client, and
runs a hand-rolled tool-use loop (≤5 iterations) using the OpenAI-compatible
function-calling API. The agent's goal: navigate to the URL, pass any
challenge, and save the page's HTML to a temp file. The agent is unaware
of images — image capture is infrastructure.
"""

from __future__ import annotations

import asyncio
import json
import logging
import tempfile
from pathlib import Path
from typing import Any

import httpx
from mcp.client.session import ClientSession
from mcp.client.stdio import StdioServerParameters, stdio_client

from app.config import get_settings

logger = logging.getLogger(__name__)

AGENT_SYSTEM_PROMPT = """\
You are a browser automation agent. Your goal: navigate to the given URL and \
save the page's full HTML to a file.

Steps:
1. Navigate to the URL.
2. If you see a Cloudflare challenge or similar, wait for it to resolve. \
You may need to change the user agent or wait for a selector.
3. Once the page has loaded, extract the full HTML content.
4. Save the HTML to the specified file path using the submit_html tool.

You have at most 5 tool calls. If you cannot reach the page, call submit_html \
with an empty string to indicate failure.

Output: call submit_html with the page's HTML content."""

MAX_ITERATIONS = 5
PLAYWRIGHT_MCP_COMMAND = "npx"
PLAYWRIGHT_MCP_ARGS = ["@playwright/mcp@latest"]
AGENT_TIMEOUT = 120.0


class AgentError(Exception):
    """Raised when the Fallback-fetch agent fails to retrieve HTML."""


async def fallback_fetch_html(url: str) -> tuple[str, dict[str, bytes]]:
    """Run the Fallback-fetch agent to retrieve HTML from a URL.

    Spawns playwright-mcp as a subprocess, connects via stdio, runs a bounded
    tool-use loop, and returns the HTML. Image bytes captured during render
    are also returned.

    Args:
        url: The URL to fetch.

    Returns:
        A tuple of ``(html, image_bytes_map)`` where ``image_bytes_map`` is
        ``{image_url: image_bytes}`` for images captured during render.

    Raises:
        AgentError: If the agent fails to retrieve HTML (iteration cap hit,
            endpoint unconfigured, or playwright-mcp unavailable).
    """
    settings = get_settings()
    if not settings.agent.is_configured:
        raise AgentError(
            "Fallback agent is not configured (set AGENT_BASE_URL and AGENT_MODEL)."
        )

    html_temp = tempfile.mktemp(suffix=".html", prefix="mcp_searxng_fallback_")
    image_bytes_map: dict[str, bytes] = {}

    try:
        html = await _run_agent_loop(url, html_temp, settings)
        if not html:
            raise AgentError(f"Agent returned empty HTML for {url}")
        image_bytes_map = await _capture_images_from_html(html, url)
        return html, image_bytes_map
    finally:
        path = Path(html_temp)
        if path.exists():
            path.unlink()


async def _capture_images_from_html(
    html: str, base_url: str
) -> dict[str, bytes]:
    """Extract image URLs from HTML and fetch their bytes via httpx.

    This is the infrastructure-level image capture for the Fallback path.
    Images that fail to fetch (e.g. behind Cloudflare) are simply skipped —
    they'll be handled by the incomplete-page retry design.

    Args:
        html: The page HTML.
        base_url: Base URL for resolving relative image URLs.

    Returns:
        Map of ``{absolute_image_url: image_bytes}`` for successfully fetched images.
    """
    from app.client import _resolve_url, fetch_image_bytes
    from bs4 import BeautifulSoup, Tag

    soup = BeautifulSoup(html, "html.parser")
    image_urls: list[str] = []
    for img_tag in soup.find_all("img"):
        if isinstance(img_tag, Tag):
            src_val = img_tag.get("src", "")
            src: str = str(src_val) if src_val else ""
            if src:
                image_urls.append(_resolve_url(src, base_url))

    results: dict[str, bytes] = {}
    for img_url in image_urls:
        img_bytes = await fetch_image_bytes(img_url, base_url)
        if img_bytes is not None:
            results[img_url] = img_bytes
    return results


async def _run_agent_loop(url: str, html_temp_path: str, settings: Any) -> str:
    """Run the bounded tool-use loop against playwright-mcp.

    Args:
        url: Target URL.
        html_temp_path: Path for the agent to save HTML.
        settings: Settings instance.

    Returns:
        The retrieved HTML string, or empty string on failure.
    """
    server_params = StdioServerParameters(
        command=PLAYWRIGHT_MCP_COMMAND,
        args=PLAYWRIGHT_MCP_ARGS,
    )

    try:
        async with stdio_client(server_params) as (read, write):
            async with ClientSession(read, write) as session:
                await session.initialize()

                tools_result = await session.list_tools()
                mcp_tools = tools_result.tools

                openai_tools = _convert_mcp_tools_to_openai(mcp_tools)

                submit_tool = {
                    "type": "function",
                    "function": {
                        "name": "submit_html",
                        "description": (
                            "Submit the retrieved HTML content. "
                            "Call this when the page is fully loaded."
                        ),
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "html": {
                                    "type": "string",
                                    "description": "The full HTML content of the page.",
                                },
                            },
                            "required": ["html"],
                        },
                    },
                }
                all_tools = openai_tools + [submit_tool]

                messages: list[dict[str, Any]] = [
                    {"role": "system", "content": AGENT_SYSTEM_PROMPT},
                    {
                        "role": "user",
                        "content": (
                            f"Navigate to {url} and save the page's HTML. "
                            f"Once you have the HTML, call submit_html with it."
                        ),
                    },
                ]

                for iteration in range(MAX_ITERATIONS):
                    assistant_response = await _call_agent_llm(
                        settings.agent, messages, all_tools
                    )

                    assistant_msg = assistant_response["choices"][0]["message"]
                    messages.append(assistant_msg)

                    tool_calls = assistant_msg.get("tool_calls")
                    if not tool_calls:
                        logger.warning(
                            "Agent returned no tool calls at iteration %d", iteration
                        )
                        break

                    for tool_call in tool_calls:
                        func = tool_call["function"]
                        tool_name = func["name"]
                        try:
                            tool_args = json.loads(func["arguments"])
                        except json.JSONDecodeError:
                            tool_args = {}

                        if tool_name == "submit_html":
                            return tool_args.get("html", "")

                        result = await session.call_tool(tool_name, tool_args)
                        result_text = _extract_tool_result_text(result)
                        messages.append({
                            "role": "tool",
                            "tool_call_id": tool_call["id"],
                            "content": result_text,
                        })

                logger.warning("Agent hit iteration cap (%d) for %s", MAX_ITERATIONS, url)
                return ""

    except Exception as exc:
        logger.error("Fallback agent error for %s: %s", url, exc)
        raise AgentError(f"Agent failed for {url}: {exc}") from exc


async def _call_agent_llm(
    role: Any, messages: list[dict[str, Any]], tools: list[dict[str, Any]]
) -> dict[str, Any]:
    """Call the agent's LLM with function-calling.

    Args:
        role: LLMRoleConfig for the agent.
        messages: Chat messages.
        tools: Tool definitions (OpenAI function-calling format).

    Returns:
        The full JSON response from the LLM.
    """
    url = f"{role.base_url.rstrip('/')}/chat/completions"
    headers: dict[str, str] = {"Content-Type": "application/json"}
    if role.api_key:
        headers["Authorization"] = f"Bearer {role.api_key}"

    payload = {
        "model": role.model,
        "messages": messages,
        "tools": tools,
        "temperature": 0,
    }

    async with httpx.AsyncClient(timeout=AGENT_TIMEOUT) as client:
        response = await client.post(url, json=payload, headers=headers)
        response.raise_for_status()
        return response.json()


def _convert_mcp_tools_to_openai(mcp_tools: Any) -> list[dict[str, Any]]:
    """Convert MCP tool definitions to OpenAI function-calling format.

    Args:
        mcp_tools: List of MCP Tool objects (from list_tools()).

    Returns:
        List of OpenAI-format tool definitions.
    """
    openai_tools: list[dict[str, Any]] = []
    for tool in mcp_tools:
        openai_tools.append({
            "type": "function",
            "function": {
                "name": tool.name,
                "description": tool.description or "",
                "parameters": tool.inputSchema or {"type": "object", "properties": {}},
            },
        })
    return openai_tools


def _extract_tool_result_text(result: Any) -> str:
    """Extract text from a CallToolResult.

    Args:
        result: CallToolResult from session.call_tool().

    Returns:
        The text content of the result, or a stringified fallback.
    """
    if hasattr(result, "content") and result.content:
        parts: list[str] = []
        for item in result.content:
            if hasattr(item, "text") and item.text:
                parts.append(item.text)
            else:
                parts.append(str(item))
        return "\n".join(parts)
    return str(result)
