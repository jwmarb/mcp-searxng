"""OpenAI-compatible LLM client for the Refiner and Captioner roles.

Uses httpx to call any OpenAI-compatible chat completions endpoint. The
Refiner is text-only; the Captioner is vision-capable (accepts image input
as base64 data URLs). Both roles fail gracefully — if the endpoint is
unconfigured or unreachable, callers receive a clear error.
"""

from __future__ import annotations

import base64
import logging
import mimetypes
from typing import Any

import httpx

from app.config import LLMRoleConfig, get_settings

logger = logging.getLogger(__name__)

REFINER_SYSTEM_PROMPT = """\
You are a Markdown refiner. Your job is to clean up a markdownified draft into \
high-quality Markdown.

Your tasks:
1. Fix broken Markdown syntax (tables, headings, lists, code blocks).
2. Remove boilerplate that survived the conversion: nav menus, cookie banners, \
related-posts sections, comment sections, footer links, share buttons, ad blocks.
3. Preserve article content VERBATIM — do not summarize, compress, rephrase, \
or drop content from the article itself.
4. Keep image markers (<!--IMG:i url=... alt=...-->) intact. You may drop \
markers that are inside boilerplate blocks (nav logos, ad images), but keep \
all markers within article content.
5. Output clean Markdown with the image markers at their correct positions.

Do NOT output any preamble, explanation, or commentary. Output ONLY the refined \
Markdown."""

CAPTIONER_SYSTEM_PROMPT = """\
You are an image captioner. Your job is to produce a comprehensive description \
of the image's visible contents.

Describe:
- What is visible in the image (objects, people, scenes).
- Any text within the image — transcribe it VERBATIM (OCR).
- Colors, layout, spatial relationships.
- The inferred image type (screenshot, photograph, chart, diagram, etc.).

If the author-provided alt text is given as context, use it as a hint but \
do not simply repeat it — describe what you actually see.

Output ONLY the description text. No preamble, no commentary."""

REFINER_MAX_TOKENS = 16384
CAPTIONER_MAX_TOKENS = 8192


class LLMError(Exception):
    """Raised when an LLM API call fails."""


async def _call_chat_completions(
    role: LLMRoleConfig,
    messages: list[dict[str, Any]],
    max_tokens: int = 4096,
) -> str:
    """Call an OpenAI-compatible chat completions endpoint.

    Args:
        role: LLM role config (base_url, api_key, model).
        messages: Chat messages list.
        max_tokens: Maximum output tokens.

    Returns:
        The assistant's text response.

    Raises:
        LLMError: If the API call fails or the role is unconfigured.
    """
    if not role.is_configured:
        raise LLMError("LLM role is not configured (set base_url and model).")

    url = f"{role.base_url.rstrip('/')}/chat/completions"
    headers: dict[str, str] = {"Content-Type": "application/json"}
    if role.api_key:
        headers["Authorization"] = f"Bearer {role.api_key}"

    payload = {
        "model": role.model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": 0,
    }

    try:
        async with httpx.AsyncClient(timeout=120.0) as client:
            response = await client.post(url, json=payload, headers=headers)
            response.raise_for_status()
            data = response.json()
            return data["choices"][0]["message"]["content"]
    except httpx.HTTPStatusError as exc:
        logger.error("LLM HTTP error for %s: %s", url, exc)
        raise LLMError(f"LLM API returned HTTP {exc.response.status_code}") from exc
    except httpx.RequestError as exc:
        logger.error("LLM request error for %s: %s", url, exc)
        raise LLMError(f"Cannot reach LLM at {url}") from exc
    except (KeyError, IndexError) as exc:
        logger.error("LLM response parse error: %s", exc)
        raise LLMError("Malformed LLM response") from exc
    except Exception as exc:
        logger.error("LLM unexpected error for %s: %s", url, exc)
        raise LLMError(f"LLM call failed: {exc}") from exc


async def refine_draft(draft: str) -> str:
    """Run the Refiner LLM on a markdownify Draft.

    Args:
        draft: The markdownify Draft with image markers.

    Returns:
        Refined Markdown with image markers preserved/dropped per the contract.

    Raises:
        LLMError: If the Refiner endpoint is unconfigured or unreachable.
    """
    settings = get_settings()
    messages = [
        {"role": "system", "content": REFINER_SYSTEM_PROMPT},
        {"role": "user", "content": draft},
    ]
    return await _call_chat_completions(
        settings.refiner, messages, max_tokens=REFINER_MAX_TOKENS
    )


async def caption_image(image_bytes: bytes, original_alt: str = "") -> str:
    """Run the Captioner LLM on an image.

    Args:
        image_bytes: Raw image bytes.
        original_alt: Author-provided alt text (passed as context).

    Returns:
        Comprehensive description of the image's visible contents.

    Raises:
        LLMError: If the Captioner endpoint is unconfigured or unreachable.
    """
    settings = get_settings()
    mime_type = _guess_mime_type(image_bytes)
    b64 = base64.b64encode(image_bytes).decode("utf-8")
    data_url = f"data:{mime_type};base64,{b64}"

    user_content: list[dict[str, Any]] = [
        {
            "type": "image_url",
            "image_url": {"url": data_url},
        }
    ]
    if original_alt:
        user_content.insert(0, {
            "type": "text",
            "text": f"Author-provided alt text: \"{original_alt}\"",
        })

    messages = [
        {"role": "system", "content": CAPTIONER_SYSTEM_PROMPT},
        {"role": "user", "content": user_content},
    ]
    return await _call_chat_completions(
        settings.captioner, messages, max_tokens=CAPTIONER_MAX_TOKENS
    )


def _guess_mime_type(image_bytes: bytes) -> str:
    """Guess MIME type from image bytes (using magic bytes)."""
    if image_bytes[:8] == b"\x89PNG\r\n\x1a\n":
        return "image/png"
    if image_bytes[:3] == b"\xff\xd8\xff":
        return "image/jpeg"
    if image_bytes[:4] == b"GIF8":
        return "image/gif"
    if image_bytes[:4] == b"RIFF" and image_bytes[8:12] == b"WEBP":
        return "image/webp"
    if image_bytes[:5] == b"<?xml" or image_bytes[:4] == b"<svg":
        return "image/svg+xml"
    return "application/octet-stream"
