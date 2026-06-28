from unittest.mock import AsyncMock, patch

import pytest

from app.tools import (
    _process_html,
    _refine_and_cache,
    _retry_missing_captions,
    web_search,
    web_url_read,
    web_url_read_from_html,
)


class TestProcessHtmlCacheHit:
    async def test_cache_hit_returns_spliced_result(self, temp_db_path):
        from app.cache import ImageCache, PageCache, hash_draft, splice_captions
        from app.config import Settings

        html = "<html><body><h1>Hello</h1></body></html>"
        settings = Settings(cache_path=temp_db_path)

        page_cache = PageCache(temp_db_path)
        image_cache = ImageCache(temp_db_path)
        await page_cache.init()
        await image_cache.init()

        from app.markdown import markdownify_with_markers
        draft, image_map = markdownify_with_markers(html)
        draft_hash = hash_draft(draft)
        refiner_output = "# Hello\n"
        await page_cache.store(draft_hash, refiner_output, {}, [])

        with patch("app.tools.get_settings", return_value=settings):
            result = await _process_html(html, None)
        assert result == "# Hello\n"

    async def test_cache_hit_with_captions(self, temp_db_path):
        from app.cache import ImageCache, PageCache, hash_draft
        from app.config import Settings
        from app.markdown import markdownify_with_markers

        html = '<html><body><p>See <img src="/cat.png" alt="cat"></p></body></html>'
        settings = Settings(cache_path=temp_db_path)

        page_cache = PageCache(temp_db_path)
        await page_cache.init()

        draft, image_map = markdownify_with_markers(html)
        draft_hash = hash_draft(draft)
        refiner_output = "<!--IMG:0-->"
        await page_cache.store(draft_hash, refiner_output, {0: "a cute cat"}, [])

        with patch("app.tools.get_settings", return_value=settings):
            result = await _process_html(html, None)
        assert "![a cute cat](/cat.png)" in result


class TestProcessHtmlCacheMiss:
    async def test_refiner_not_configured_returns_draft(self, temp_db_path):
        from app.config import Settings

        html = "<html><body><h1>Hello</h1></body></html>"
        settings = Settings(cache_path=temp_db_path)

        with patch("app.tools.get_settings", return_value=settings):
            result = await _process_html(html, None)
        from app.markdown import markdownify_with_markers
        draft, _ = markdownify_with_markers(html)
        assert result == draft

    async def test_refine_and_cache_stores_entry(self, temp_db_path):
        from app.cache import ImageCache, PageCache, hash_draft
        from app.config import LLMRoleConfig, Settings
        from app.markdown import markdownify_with_markers

        html = "<html><body><h1>Test</h1></body></html>"
        settings = Settings(
            cache_path=temp_db_path,
            refiner=LLMRoleConfig(base_url="http://fake/v1", api_key="k", model="m"),
        )

        draft, image_map = markdownify_with_markers(html)
        draft_hash = hash_draft(draft)

        page_cache = PageCache(temp_db_path)
        image_cache = ImageCache(temp_db_path)
        await page_cache.init()
        await image_cache.init()

        with patch("app.tools.get_settings", return_value=settings):
            with patch("app.tools.refine_draft", new_callable=AsyncMock) as mock_refine:
                mock_refine.return_value = "# Test\n"
                result = await _refine_and_cache(
                    draft, draft_hash, image_map, None, page_cache, image_cache
                )
                assert "# Test" in result

        entry = await page_cache.get(draft_hash)
        assert entry is not None
        assert entry.refiner_output == "# Test\n"


class TestProcessHtmlIncompleteRetry:
    async def test_retries_missing_captions(self, temp_db_path):
        from app.cache import ImageCache, PageCache, PageCacheEntry, hash_draft
        from app.config import LLMRoleConfig, Settings
        from app.markdown import markdownify_with_markers

        html = '<html><body><img src="/img.png"></body></html>'
        settings = Settings(
            cache_path=temp_db_path,
            captioner=LLMRoleConfig(base_url="http://fake/v1", api_key="k", model="m"),
        )

        draft, image_map = markdownify_with_markers(html)
        draft_hash = hash_draft(draft)

        page_cache = PageCache(temp_db_path)
        image_cache = ImageCache(temp_db_path)
        await page_cache.init()
        await image_cache.init()

        refiner_output = "<!--IMG:0-->"
        await page_cache.store(draft_hash, refiner_output, {}, [0])

        entry = PageCacheEntry(
            hash=draft_hash,
            refiner_output=refiner_output,
            captions={},
            missing=[0],
            created_at="2024-01-01",
        )

        with patch("app.tools.get_settings", return_value=settings):
            with patch("app.tools.fetch_image_bytes", new_callable=AsyncMock) as mock_fetch:
                mock_fetch.return_value = b"\x89PNG\r\n\x1a\n" + b"\x00" * 100
                with patch("app.tools.caption_image", new_callable=AsyncMock) as mock_cap:
                    mock_cap.return_value = "a test image"
                    updated = await _retry_missing_captions(entry, image_map, None, image_cache)
                    assert 0 in updated
                    assert updated[0] == "a test image"

    async def test_retry_skips_when_captioner_unconfigured(self, temp_db_path):
        from app.cache import ImageCache, PageCache, PageCacheEntry, hash_draft
        from app.config import Settings
        from app.markdown import markdownify_with_markers

        html = '<html><body><img src="/img.png"></body></html>'
        settings = Settings(cache_path=temp_db_path)

        draft, image_map = markdownify_with_markers(html)
        draft_hash = hash_draft(draft)

        image_cache = ImageCache(temp_db_path)
        await image_cache.init()

        entry = PageCacheEntry(
            hash=draft_hash,
            refiner_output="<!--IMG:0-->",
            captions={},
            missing=[0],
            created_at="2024-01-01",
        )

        with patch("app.tools.get_settings", return_value=settings):
            updated = await _retry_missing_captions(entry, image_map, None, image_cache)
        assert updated == {}


class TestWebUrlRead:
    async def test_fetch_error_returns_error_string(self):
        with patch("app.tools.fetch_html", side_effect=Exception("connection refused")):
            result = await web_url_read("http://example.com")
            assert "Error fetching" in result

    async def test_success_delegates_to_process_html(self):
        with patch("app.tools.fetch_html", new_callable=AsyncMock) as mock_fetch:
            with patch("app.tools._process_html", new_callable=AsyncMock) as mock_process:
                mock_fetch.return_value = "<html>ok</html>"
                mock_process.return_value = "# Refined\n"
                result = await web_url_read("http://example.com")
                assert result == "# Refined\n"
                mock_process.assert_called_once_with("<html>ok</html>", "http://example.com")


class TestWebUrlReadFromHtml:
    async def test_delegates_to_process_html(self):
        with patch("app.tools._process_html", new_callable=AsyncMock) as mock_process:
            mock_process.return_value = "# From HTML\n"
            result = await web_url_read_from_html("<html><body>test</body></html>")
            assert result == "# From HTML\n"
            mock_process.assert_called_once()
            call_args = mock_process.call_args
            assert call_args[0][0] == "<html><body>test</body></html>"
            assert call_args[0][1] is None


class TestWebSearch:
    async def test_delegates_to_search(self):
        with patch("app.tools.search", new_callable=AsyncMock) as mock_search:
            mock_search.return_value = "Title: Result\nURL: http://x\n"
            result = await web_search("test query", count=5)
            assert result == "Title: Result\nURL: http://x\n"
            mock_search.assert_called_once_with("test query", limit=5)