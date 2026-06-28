"""Custom markdownify converter that emits positional image markers.

Instead of converting ``<img>`` to ``![alt](url)``, this converter emits
``<!--IMG:i url=... alt=...-->`` and records a side map of
``{i: {url, alt}}`` for each image in document order.

The Refiner sees markers with context (url, alt) for boilerplate judgment.
Before splice, :func:`strip_marker_attributes` removes the url/alt attributes,
leaving clean ``<!--IMG:i-->`` markers for the splice step to replace with
``![caption_i](url)``.
"""

from __future__ import annotations

import re
from typing import Any

from markdownify import MarkdownConverter

_MARKER_RE = re.compile(r"<!--IMG:(\d+)(?:\s+url=(.*?)\s+alt=(.*?))?-->")
_STRIP_RE = re.compile(r"<!--IMG:(\d+)\s+url=.*?\s+alt=.*?-->")
_CLEAN_MARKER_RE = re.compile(r"<!--IMG:(\d+)-->")


class ImageMarkerConverter(MarkdownConverter):
    """MarkdownConverter subclass that emits image markers instead of ``![alt](url)``."""

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        super().__init__(*args, **kwargs)
        self.image_map: dict[int, dict[str, str]] = {}
        self._image_counter = 0

    def convert_img(self, el, text, parent_tags):  # noqa: ANN001
        """Override image conversion to emit positional markers.

        Args:
            el: BeautifulSoup element (the ``<img>`` tag).
            text: Inner text (unused for images).
            parent_tags: Set of parent tag context.

        Returns:
            An HTML comment marker ``<!--IMG:i url=... alt=...-->``.
        """
        src = el.attrs.get("src", None) or ""
        alt = el.attrs.get("alt", None) or ""

        # Sanitize values to avoid breaking the HTML comment.
        safe_url = src.replace("-->", "__END_COMMENT__")
        safe_alt = alt.replace("-->", "__END_COMMENT__")

        index = self._image_counter
        self.image_map[index] = {"url": src, "alt": alt}
        self._image_counter += 1

        return f"<!--IMG:{index} url={safe_url} alt={safe_alt}-->"


def markdownify_with_markers(html: str) -> tuple[str, dict[int, dict[str, str]]]:
    """Convert HTML to Markdown with positional image markers.

    Args:
        html: Raw HTML string.

    Returns:
        A tuple of ``(draft_markdown, image_map)`` where ``image_map`` is
        ``{index: {url: str, alt: str}}`` for each ``<img>`` in document order.
    """
    converter = ImageMarkerConverter()
    draft = converter.convert(html)
    return draft, converter.image_map


def strip_marker_attributes(markdown: str) -> str:
    """Strip ``url=... alt=...`` attributes from image markers.

    Transforms ``<!--IMG:i url=... alt=...-->`` into ``<!--IMG:i-->``,
    leaving clean markers for the splice step.

    Args:
        markdown: Markdown text with attributed image markers.

    Returns:
        Markdown text with clean ``<!--IMG:i-->`` markers.
    """
    def _replace(match: re.Match) -> str:
        index = match.group(1)
        return f"<!--IMG:{index}-->"

    return _STRIP_RE.sub(_replace, markdown)


def extract_image_indices(refiner_output: str) -> list[int]:
    """Extract all image indices from ``<!--IMG:i-->`` markers in the text.

    After the Refiner runs, it may have dropped some boilerplate images.
    This function returns the indices of images the Refiner kept, in
    document order.

    Args:
        refiner_output: The Refiner's output Markdown with markers.

    Returns:
        Sorted list of image indices that the Refiner kept.
    """
    indices: list[int] = []
    for match in _MARKER_RE.finditer(refiner_output):
        indices.append(int(match.group(1)))
    return sorted(indices)
