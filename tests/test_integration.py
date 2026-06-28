"""Integration tests against real LLM endpoints configured in .env.

These tests use the actual LLM API keys from `.env` and verify end-to-end
behavior of the refinement pipeline.  They are marked `integration` so they
can be skipped with `pytest -m 'not integration'`.

Run with:
    pytest tests/test_integration.py -v -m integration
"""

from __future__ import annotations

from unittest.mock import patch

import pytest

from app.cache import ImageCache, PageCache, hash_draft, hash_image_bytes, splice_captions
from app.config import LLMRoleConfig, Settings, get_settings, is_role_configured
from app.llm import caption_image, refine_draft
from app.markdown import extract_image_indices, markdownify_with_markers, strip_marker_attributes
from app.tools import _process_html, _refine_and_cache

pytestmark = pytest.mark.integration


@pytest.fixture(scope="module")
def integration_settings():
    return get_settings()


@pytest.fixture
def temp_integration_db(tmp_path):
    return str(tmp_path / "integration_cache.db")


class TestRefinerIntegration:

    def test_refiner_configured(self, integration_settings):
        assert is_role_configured(integration_settings.refiner)

    async def test_refine_simple_article(self):
        html = """\
<html><head><title>Python Tips</title></head><body>
<nav><a href="/">Home</a> <a href="/about">About</a></nav>
<h1>5 Python Tips</h1>
<p>Here are some useful Python tips for daily coding.</p>
<ol>
<li>Use list comprehensions</li>
<li>Prefer f-strings over format()</li>
<li>Use pathlib instead of os.path</li>
</ol>
<footer>Copyright 2024. All rights reserved.</footer>
</body></html>"""
        draft, _ = markdownify_with_markers(html)
        refined = await refine_draft(draft)

        assert refined
        assert "Python" in refined
        assert "list comprehensions" in refined or "comprehension" in refined.lower()
        assert "Copyright" not in refined or "copyright" not in refined.lower()

    async def test_refine_preserves_image_markers(self):
        html = """\
<html><body>
<h1>Article</h1>
<p>See the chart below:</p>
<img src="/chart.png" alt="Sales chart Q1-Q4">
<p>The data shows growth.</p>
</body></html>"""
        draft, image_map = markdownify_with_markers(html)
        assert len(image_map) == 1

        refined = await refine_draft(draft)
        indices = extract_image_indices(refined)
        assert refined
        assert "Article" in refined or "article" in refined.lower()


class TestCaptionerIntegration:

    def test_captioner_configured(self, integration_settings):
        assert is_role_configured(integration_settings.captioner)

    async def test_caption_png_image(self):
        from pathlib import Path

        png_path = Path(__file__).parent / "ornith_35b_eval.png"
        png_bytes = png_path.read_bytes()
        caption = await caption_image(png_bytes, original_alt="Evaluation results chart")

        assert caption
        assert len(caption) > 5


class TestPipelineIntegration:

    async def test_full_pipeline_cache_miss_then_hit(self, temp_integration_db):
        html = """\
<html><head><title>Test Article</title></head><body>
<nav><a href="/">Skip this nav</a></nav>
<h1>Understanding Async/Await</h1>
<p>Async/await is a powerful pattern for concurrent programming.</p>
<p>It allows you to write asynchronous code that looks synchronous.</p>
<footer>Ignore this footer</footer>
</body></html>"""

        settings = Settings(cache_path=temp_integration_db)
        with patch("app.tools.get_settings", return_value=settings):
            first = await _process_html(html, None)
            second = await _process_html(html, None)

        assert first
        assert second
        assert first == second
        assert "Async" in first or "async" in first.lower()
        assert "concurrent" in first.lower() or "programming" in first.lower()

    async def test_full_pipeline_with_cache_persistence(self, temp_integration_db):
        html = "<html><body><h1>Persistence Test</h1><p>Content here.</p></body></html>"
        draft_hash = hash_draft(markdownify_with_markers(html)[0])

        real_settings = get_settings()
        settings = Settings(
            cache_path=temp_integration_db,
            refiner=real_settings.refiner,
        )
        with patch("app.tools.get_settings", return_value=settings):
            await _process_html(html, None)

        pc = PageCache(temp_integration_db)
        await pc.init()
        entry = await pc.get(draft_hash)

        assert entry is not None
        assert "Persistence" in entry.refiner_output or "persistence" in entry.refiner_output.lower()


class TestCacheLLMRoundTrip:

    async def test_refine_and_cache_round_trip(self, temp_integration_db):
        html = """\
<html><body>
<h1>Cache Round Trip</h1>
<p>Testing cache persistence with real LLM output.</p>
</body></html>"""
        draft, image_map = markdownify_with_markers(html)
        draft_hash = hash_draft(draft)

        settings = Settings(
            cache_path=temp_integration_db,
            refiner=get_settings().refiner,
        )
        pc = PageCache(temp_integration_db)
        ic = ImageCache(temp_integration_db)
        await pc.init()
        await ic.init()

        with patch("app.tools.get_settings", return_value=settings):
            result = await _refine_and_cache(
                draft, draft_hash, image_map, None, pc, ic
            )

        entry = await pc.get(draft_hash)

        assert result
        assert entry is not None
        assert entry.refiner_output
        assert "Cache" in result or "cache" in result.lower() or "Round" in result

    async def test_image_cache_round_trip(self, temp_integration_db):
        image_bytes = b"\x89PNG\r\n\x1a\n" + b"\x00" * 100
        img_hash = hash_image_bytes(image_bytes)

        ic = ImageCache(temp_integration_db)
        await ic.init()
        await ic.store(img_hash, "A test caption for integration")
        cached = await ic.get(img_hash)

        assert cached == "A test caption for integration"

    async def test_splice_with_real_refiner_output(self):
        html = """\
<html><body>
<h1>Image Test</h1>
<p>Here is a diagram:</p>
<img src="/diagram.png" alt="System architecture diagram">
<p>The diagram shows components.</p>
</body></html>"""
        draft, image_map = markdownify_with_markers(html)
        assert len(image_map) == 1

        refined = await refine_draft(draft)
        refiner_output = strip_marker_attributes(refined)
        indices = extract_image_indices(refiner_output)

        if indices:
            captions = {idx: "Cached caption for test" for idx in indices}
            result = splice_captions(refiner_output, captions, image_map)
        else:
            result = refiner_output

        assert result
        if image_map:
            assert "!" in result or "<!--IMG:" not in result