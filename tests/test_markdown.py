import pytest

from app.markdown import (
    extract_image_indices,
    markdownify_with_markers,
    strip_marker_attributes,
)


class TestMarkdownifyWithMarkers:
    def test_no_images(self, sample_html_simple):
        draft, image_map = markdownify_with_markers(sample_html_simple)
        assert "Hello World" in draft
        assert "This is a test paragraph" in draft
        assert len(image_map) == 0

    def test_images_emitted_as_markers(self, sample_html_with_images):
        draft, image_map = markdownify_with_markers(sample_html_with_images)
        assert "<!--IMG:0" in draft
        assert "<!--IMG:1" in draft
        assert "<!--IMG:2" in draft
        assert len(image_map) == 3

    def test_image_map_has_url_and_alt(self, sample_html_with_images):
        _, image_map = markdownify_with_markers(sample_html_with_images)
        assert image_map[0]["url"] == "/content/figure1.png"
        assert image_map[0]["alt"] == "Figure 1"
        assert image_map[1]["url"] == "/assets/logo.png"
        assert image_map[1]["alt"] == "Site Logo"

    def test_image_indices_are_sequential(self, sample_html_with_images):
        _, image_map = markdownify_with_markers(sample_html_with_images)
        assert list(image_map.keys()) == [0, 1, 2]

    def test_sanitizes_comment_end_in_url(self):
        html = '<img src="a-->b.png" alt="x-->">'
        draft, _ = markdownify_with_markers(html)
        assert "__END_COMMENT__" in draft
        assert "a-->b" not in draft

    def test_empty_html(self):
        draft, image_map = markdownify_with_markers("")
        assert draft == ""
        assert len(image_map) == 0


class TestStripMarkerAttributes:
    def test_strips_url_and_alt(self):
        md = "Hello <!--IMG:0 url=/cat.png alt=Figure 1--> world"
        result = strip_marker_attributes(md)
        assert "<!--IMG:0-->" in result
        assert "url=" not in result

    def test_multiple_markers(self):
        md = "<!--IMG:0 url=/a.png alt=a--><!--IMG:1 url=/b.png alt=b-->"
        result = strip_marker_attributes(md)
        assert "<!--IMG:0-->" in result
        assert "<!--IMG:1-->" in result

    def test_already_clean_markers_unchanged(self):
        md = "<!--IMG:0--> and <!--IMG:1-->"
        result = strip_marker_attributes(md)
        assert result == md

    def test_no_markers_unchanged(self):
        md = "Just text."
        assert strip_marker_attributes(md) == "Just text."


class TestExtractImageIndices:
    def test_extracts_all_indices(self):
        text = "Hello <!--IMG:0--> world <!--IMG:2--> end <!--IMG:1-->"
        indices = extract_image_indices(text)
        assert indices == [0, 1, 2]

    def test_returns_sorted(self):
        text = "<!--IMG:5--><!--IMG:2--><!--IMG:8-->"
        assert extract_image_indices(text) == [2, 5, 8]

    def test_no_markers_returns_empty(self):
        assert extract_image_indices("plain text") == []

    def test_markers_with_attributes(self):
        text = "<!--IMG:3 url=/x.png alt=foo-->"
        assert extract_image_indices(text) == [3]

    def test_deduplicates_positions(self):
        text = "<!--IMG:0--><!--IMG:0-->"
        indices = extract_image_indices(text)
        assert indices == [0, 0]