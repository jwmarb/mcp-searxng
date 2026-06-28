from unittest.mock import AsyncMock, patch

import httpx
import json
import pytest

from app.client import (
    _primary_fetch,
    _resolve_url,
    fetch_image_bytes,
    fetch_html,
    search,
)


class TestResolveUrl:
    def test_absolute_http_unchanged(self):
        assert _resolve_url("http://example.com/img.png") == "http://example.com/img.png"

    def test_absolute_https_unchanged(self):
        assert _resolve_url("https://example.com/img.png") == "https://example.com/img.png"

    def test_data_url_unchanged(self):
        assert _resolve_url("data:image/png;base64,abc") == "data:image/png;base64,abc"

    def test_relative_resolved_against_base(self):
        result = _resolve_url("/img.png", "https://example.com/page")
        assert result == "https://example.com/img.png"

    def test_relative_path_resolved(self):
        result = _resolve_url("assets/img.png", "https://example.com/page/")
        assert result == "https://example.com/page/assets/img.png"

    def test_relative_no_base_returns_as_is(self):
        assert _resolve_url("/img.png", None) == "/img.png"

    def test_relative_no_base_string(self):
        assert _resolve_url("img.png") == "img.png"


class TestPrimaryFetch:
    async def test_success(self):
        with patch("app.client.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.get.return_value = AsyncMock(
                status_code=200,
                text="<html><body>Hello</body></html>",
                raise_for_status=lambda: None,
            )
            MockClient.return_value.__aenter__.return_value = instance
            result = await _primary_fetch("http://example.com")
            assert result == "<html><body>Hello</body></html>"

    async def test_http_error_returns_none(self):
        with patch("app.client.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.get.side_effect = httpx.HTTPStatusError(
                "403",
                request=httpx.Request("GET", "http://example.com"),
                response=httpx.Response(403),
            )
            MockClient.return_value.__aenter__.return_value = instance
            result = await _primary_fetch("http://example.com")
            assert result is None

    async def test_connection_error_returns_none(self):
        with patch("app.client.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.get.side_effect = httpx.ConnectError("refused")
            MockClient.return_value.__aenter__.return_value = instance
            result = await _primary_fetch("http://example.com")
            assert result is None


class TestFetchImageBytes:
    async def test_success(self):
        with patch("app.client.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.get.return_value = AsyncMock(
                status_code=200,
                content=b"\x89PNG\r\n\x1a\n",
                raise_for_status=lambda: None,
            )
            MockClient.return_value.__aenter__.return_value = instance
            result = await fetch_image_bytes("http://example.com/img.png")
            assert result == b"\x89PNG\r\n\x1a\n"

    async def test_failure_returns_none(self):
        with patch("app.client.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.get.side_effect = httpx.ConnectError("refused")
            MockClient.return_value.__aenter__.return_value = instance
            result = await fetch_image_bytes("http://example.com/img.png")
            assert result is None

    async def test_relative_url_resolved(self):
        with patch("app.client.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.get.return_value = AsyncMock(
                status_code=200,
                content=b"\x89PNG",
                raise_for_status=lambda: None,
            )
            MockClient.return_value.__aenter__.return_value = instance
            await fetch_image_bytes("/img.png", "https://example.com/")
            call_url = instance.get.call_args[0][0]
            assert call_url == "https://example.com/img.png"


class TestFetchHtml:
    async def test_primary_success(self):
        with patch("app.client._primary_fetch", new_callable=AsyncMock) as mock_primary:
            mock_primary.return_value = "<html>ok</html>"
            result = await fetch_html("http://example.com")
            assert result == "<html>ok</html>"
            mock_primary.assert_called_once_with("http://example.com")

    async def test_primary_falls_back_to_agent(self):
        with patch("app.client._primary_fetch", new_callable=AsyncMock) as mock_primary:
            with patch("app.client.fallback_fetch_html", new_callable=AsyncMock) as mock_fallback:
                mock_primary.return_value = None
                mock_fallback.return_value = ("<html>fallback</html>", {})
                result = await fetch_html("http://example.com")
                assert result == "<html>fallback</html>"
                mock_fallback.assert_called_once_with("http://example.com")


def _make_search_response(results, query="test"):
    return {
        "query": query,
        "number_of_results": len(results),
        "results": results,
        "infoboxes": [],
        "answers": [],
        "corrections": [],
        "suggestions": [],
        "unresponsive_engines": [],
    }


class TestSearch:
    async def test_basic_search(self):
        from app.config import Settings

        settings = Settings()
        with patch("app.client.get_settings", return_value=settings):
            with patch("app.client.httpx.AsyncClient") as MockClient:
                instance = AsyncMock()
                mock_response = _make_search_response([
                    {"title": "Result 1", "url": "http://example.com/1", "content": "Content 1"},
                    {"title": "Result 2", "url": "http://example.com/2", "content": "Content 2"},
                ])
                instance.get.return_value = AsyncMock(
                    status_code=200,
                    text=json.dumps(mock_response),
                    raise_for_status=lambda: None,
                )
                MockClient.return_value.__aenter__.return_value = instance
                result = await search("test query", limit=2)
                assert "Result 1" in result
                assert "Result 2" in result

    async def test_search_respects_limit(self):
        from app.config import Settings

        settings = Settings()
        with patch("app.client.get_settings", return_value=settings):
            with patch("app.client.httpx.AsyncClient") as MockClient:
                instance = AsyncMock()
                results = [
                    {"title": f"Result {i}", "url": f"http://example.com/{i}", "content": f"Content {i}"}
                    for i in range(5)
                ]
                mock_response = _make_search_response(results)
                instance.get.return_value = AsyncMock(
                    status_code=200,
                    text=json.dumps(mock_response),
                    raise_for_status=lambda: None,
                )
                MockClient.return_value.__aenter__.return_value = instance
                result = await search("test", limit=2)
                assert "Result 0" in result
                assert "Result 1" in result
                assert "Result 2" not in result

    async def test_search_no_results(self):
        from app.config import Settings

        settings = Settings()
        with patch("app.client.get_settings", return_value=settings):
            with patch("app.client.httpx.AsyncClient") as MockClient:
                instance = AsyncMock()
                instance.get.return_value = AsyncMock(
                    status_code=200,
                    text=json.dumps(_make_search_response([])),
                    raise_for_status=lambda: None,
                )
                MockClient.return_value.__aenter__.return_value = instance
                result = await search("nothing")
                assert "No results found" in result