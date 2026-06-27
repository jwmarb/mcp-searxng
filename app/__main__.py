"""CLI entry point for the MCP-SearXNG SSE server."""

import argparse
import logging
import os

import uvicorn

from app import mcp  # noqa: F401
from app import tools  # noqa: F401  — registers @mcp.tool() decorators
from app.server import create_starlette_app

logging.basicConfig(level=logging.INFO)


def main() -> None:
    """Run the MCP-SearXNG SSE-based server."""
    parser = argparse.ArgumentParser(
        description="Run MCP-SearXNG SSE-based server"
    )
    parser.add_argument("--host", default="0.0.0.0", help="Host to bind to")
    parser.add_argument("--port", type=int, default=5488, help="Port to listen on")
    parser.add_argument(
        "--searxng_url",
        default="http://localhost:8888",
        help="SearXNG URL to connect to",
    )

    args = parser.parse_args()

    if os.environ.get("SEARXNG_URL") is None:
        os.environ["SEARXNG_URL"] = args.searxng_url

    mcp_server = mcp._mcp_server  # noqa: SLF001
    starlette_app = create_starlette_app(mcp_server, debug=True)

    uvicorn.run(starlette_app, host=args.host, port=args.port)


if __name__ == "__main__":
    main()