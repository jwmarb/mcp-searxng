"""MCP tool implementations for SearXNG with full refinement pipeline."""

from __future__ import annotations

import asyncio
import logging

from app import mcp
from app.agent import AgentError
from app.cache import ImageCache, PageCache, hash_draft, hash_image_bytes, splice_captions
from app.client import fetch_html, fetch_image_bytes, search
from app.config import get_settings, is_role_configured
from app.llm import LLMError, caption_image, refine_draft
from app.markdown import (
    extract_image_indices,
    markdownify_with_markers,
    strip_marker_attributes,
)

logger = logging.getLogger(__name__)


@mcp.tool()
async def web_search(query: str, count: int = 3) -> str:
    """Performs a web search using the SearXNG API.

    Ideal for general queries, news, articles, and online content.
    Use this for broad information gathering, recent events,
    or when you need diverse web sources.
    """
    return await search(query, limit=count)


@mcp.tool()
async def web_url_read(url: str) -> str:
    """Read the content from a URL and return it as refined Markdown.

    Runs the full pipeline: fetch HTML (Primary httpx or Fallback agent),
    convert to Markdown, check cache, refine via LLM, caption images, and
    return. Cached pages return instantly without LLM calls.
    """
    try:
        html = await fetch_html(url)
    except AgentError as exc:
        logger.error("Fallback agent failed for %s: %s", url, exc)
        return f"Error fetching URL {url}: {exc}"
    except Exception as exc:
        logger.error("Failed to fetch %s: %s", url, exc)
        return f"Error fetching URL {url}: {exc}"

    return await _process_html(html, url)


@mcp.tool()
async def web_url_read_from_html(html: str) -> str:
    """Convert raw HTML to refined Markdown with image captions.

    Use this when you already have the HTML (e.g. fetched via your own
    browser). Runs the same refinement and caching pipeline as
    web_url_read but skips the fetch step.
    """
    return await _process_html(html, None)


async def _process_html(html: str, base_url: str | None) -> str:
    """Run the full refinement pipeline on HTML.

    Args:
        html: Raw HTML content.
        base_url: Base URL for resolving relative image URLs (None if unknown).

    Returns:
        Refined Markdown with image captions.
    """
    draft, image_map = markdownify_with_markers(html)
    draft_hash = hash_draft(draft)

    settings = get_settings()
    db_path = settings.cache_path

    page_cache = PageCache(db_path)
    image_cache = ImageCache(db_path)
    await page_cache.init()
    await image_cache.init()

    entry = await page_cache.get(draft_hash)
    if entry is not None:
        if entry.missing:
            updated_captions = await _retry_missing_captions(
                entry, image_map, base_url, image_cache
            )
            return splice_captions(entry.refiner_output, updated_captions, image_map)
        return splice_captions(entry.refiner_output, entry.captions, image_map)

    return await _refine_and_cache(
        draft, draft_hash, image_map, base_url, page_cache, image_cache
    )


async def _retry_missing_captions(
    entry, image_map, base_url, image_cache
) -> dict[int, str]:
    """Retry captioning for missing images on an incomplete cache entry.

    Skips the Refiner (reuses cached marker'd output). Only re-captions
    the missing image indices. Checks Image cache first.

    Returns:
        Updated captions dict (entry.captions + newly captioned images).
    """
    if not is_role_configured(get_settings().captioner):
        return dict(entry.captions)

    updated_captions = dict(entry.captions)
    still_missing: list[int] = []

    for idx in entry.missing:
        img_info = image_map.get(idx)
        if not img_info or not img_info.get("url"):
            still_missing.append(idx)
            continue

        image_bytes = await fetch_image_bytes(img_info["url"], base_url)
        if image_bytes is None:
            still_missing.append(idx)
            continue

        img_hash = hash_image_bytes(image_bytes)
        cached_caption = await image_cache.get(img_hash)
        if cached_caption is not None:
            updated_captions[idx] = cached_caption
            continue

        try:
            caption = await caption_image(image_bytes, img_info.get("alt", ""))
            await image_cache.store(img_hash, caption)
            updated_captions[idx] = caption
        except LLMError as exc:
            logger.warning("Captioner failed for image %d: %s", idx, exc)
            still_missing.append(idx)

    await page_cache_update_captions_safe(
        entry.hash, updated_captions, still_missing
    )
    return updated_captions


async def page_cache_update_captions_safe(
    hash_val: str, captions: dict[int, str], missing: list[int]
) -> None:
    """Update captions in PageCache, handling errors gracefully."""
    settings = get_settings()
    pc = PageCache(settings.cache_path)
    await pc.update_captions(hash_val, captions, missing)


async def _refine_and_cache(
    draft: str,
    draft_hash: str,
    image_map: dict[int, dict[str, str]],
    base_url: str | None,
    page_cache: PageCache,
    image_cache: ImageCache,
) -> str:
    """Run Refiner + Captioner in parallel, splice, cache, and return."""
    settings = get_settings()
    refiner_configured = is_role_configured(settings.refiner)
    captioner_configured = is_role_configured(settings.captioner)

    if not refiner_configured:
        logger.warning("Refiner not configured, returning unrefined Draft")
        return draft

    refiner_task = asyncio.create_task(_run_refiner(draft))
    kept_indices = extract_image_indices(
        strip_marker_attributes(draft)
    )

    caption_task = asyncio.create_task(
        _caption_all_images(
            [i for i in kept_indices if i in image_map],
            image_map,
            base_url,
            image_cache,
            captioner_configured,
        )
    )

    try:
        refined_output, captions_results = await asyncio.gather(
            refiner_task, caption_task
        )
    except LLMError as exc:
        logger.error("Refiner failed, returning unrefined Draft: %s", exc)
        return draft

    refiner_output = strip_marker_attributes(refined_output)
    kept_after_refiner = extract_image_indices(refiner_output)

    captions: dict[int, str] = {}
    missing: list[int] = []
    for idx in kept_after_refiner:
        if idx in captions_results:
            captions[idx] = captions_results[idx]
        else:
            missing.append(idx)

    await page_cache.store(draft_hash, refiner_output, captions, missing)

    return splice_captions(refiner_output, captions, image_map)


async def _run_refiner(draft: str) -> str:
    """Run the Refiner LLM, propagating errors for the caller to handle."""
    return await refine_draft(draft)


async def _caption_all_images(
    indices: list[int],
    image_map: dict[int, dict[str, str]],
    base_url: str | None,
    image_cache: ImageCache,
    captioner_configured: bool,
) -> dict[int, str]:
    """Caption all images in parallel, checking Image cache first.

    Returns:
        Map of ``{image_index: caption}`` for successfully captioned images.
    """
    if not captioner_configured:
        logger.warning("Captioner not configured, skipping image captioning")
        return {}

    async def _caption_one(idx: int) -> tuple[int, str | None]:
        img_info = image_map.get(idx)
        if not img_info or not img_info.get("url"):
            return idx, None

        image_bytes = await fetch_image_bytes(img_info["url"], base_url)
        if image_bytes is None:
            return idx, None

        img_hash = hash_image_bytes(image_bytes)
        cached = await image_cache.get(img_hash)
        if cached is not None:
            return idx, cached

        try:
            caption = await caption_image(image_bytes, img_info.get("alt", ""))
            await image_cache.store(img_hash, caption)
            return idx, caption
        except LLMError as exc:
            logger.warning("Captioner failed for image %d: %s", idx, exc)
            return idx, None

    results = await asyncio.gather(*[_caption_one(i) for i in indices])
    return {idx: cap for idx, cap in results if cap is not None}
