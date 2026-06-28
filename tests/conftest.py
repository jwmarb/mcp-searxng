"""Shared test fixtures for mcp-searxng tests."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

# Clear all LLM env vars before importing app modules so tests start clean.
for key in list(os.environ):
    if key.startswith(("REFINER_", "CAPTIONER_", "AGENT_", "SEARXNG_")):
        del os.environ[key]


@pytest.fixture
def temp_db_path(tmp_path: Path) -> str:
    """Return a path for a temporary SQLite database file."""
    return str(tmp_path / "test_cache.db")


@pytest.fixture
def sample_html_simple() -> str:
    """Minimal HTML with no images."""
    return """\
<html><head><title>Simple</title></head><body>
<h1>Hello World</h1>
<p>This is a test paragraph.</p>
</body></html>"""


@pytest.fixture
def sample_html_with_images() -> str:
    """HTML with multiple images including different alt text."""
    return """\
<html><head><title>Test</title></head><body>
<h1>Article Title</h1>
<p>Some text <img src="/content/figure1.png" alt="Figure 1"> here.</p>
<nav><img src="/assets/logo.png" alt="Site Logo"></nav>
<p>More text</p>
<img src="/content/figure2.png" alt="">
<p>End</p>
</body></html>"""


@pytest.fixture
def sample_html_boilerplate() -> str:
    """HTML with boilerplate blocks (nav, footer) that the Refiner should drop."""
    return """\
<html><head><title>Boilerplate Test</title></head><body>
<nav><a href="/">Home</a> <a href="/about">About</a></nav>
<h1>Main Article</h1>
<p>This is the article body that should be preserved.</p>
<footer>Copyright 2024. All rights reserved.</footer>
</body></html>"""


@pytest.fixture
def mock_refiner_configured():
    """Patch settings so the Refiner appears configured with a fake endpoint."""
    with patch("app.config.get_settings") as mock:
        settings = MagicMock()
        settings.refiner.base_url = "http://fake-llm:8080/v1"
        settings.refiner.api_key = "test-key"
        settings.refiner.model = "test-refiner"
        settings.refiner.is_configured = True
        settings.captioner.base_url = ""
        settings.captioner.api_key = ""
        settings.captioner.model = ""
        settings.captioner.is_configured = False
        settings.agent.base_url = ""
        settings.agent.api_key = ""
        settings.agent.model = ""
        settings.agent.is_configured = False
        settings.cache_path = ":memory:"
        settings.searxng_url = "http://localhost:8888"
        mock.return_value = settings
        yield settings


@pytest.fixture
def mock_captioner_configured(mock_refiner_configured):
    """Also configure the Captioner."""
    mock_refiner_configured.captioner.base_url = "http://fake-llm:8080/v1"
    mock_refiner_configured.captioner.api_key = "test-key"
    mock_refiner_configured.captioner.model = "test-captioner"
    mock_refiner_configured.captioner.is_configured = True
    yield mock_refiner_configured


@pytest.fixture
def mock_llm_refiner_response():
    """Mock the refine_draft function to return a fixed refined markdown."""
    with patch("app.tools.refine_draft", new_callable=AsyncMock) as mock:
        mock.return_value = "# Refined Title\n\nRefined content.\n"
        yield mock


@pytest.fixture
def mock_llm_captioner_response():
    """Mock the caption_image function to return a fixed caption."""
    with patch("app.tools.caption_image", new_callable=AsyncMock) as mock:
        mock.return_value = "A test image caption"
        yield mock


@pytest.fixture
def mock_fetch_image_bytes():
    """Mock fetch_image_bytes to return dummy image data."""
    with patch("app.tools.fetch_image_bytes", new_callable=AsyncMock) as mock:
        mock.return_value = b"\x89PNG\r\n\x1a\n" + b"\x00" * 100
        yield mock
