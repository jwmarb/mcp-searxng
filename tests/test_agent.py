from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from app.agent import (
    AgentError,
    _capture_images_from_html,
    _convert_mcp_tools_to_openai,
    _extract_tool_result_text,
)


class TestAgentError:
    def test_is_exception(self):
        assert issubclass(AgentError, Exception)

    def test_message_preserved(self):
        err = AgentError("something went wrong")
        assert str(err) == "something went wrong"


class TestConvertMcpToolsToOpenai:
    def test_converts_single_tool(self):
        tool = MagicMock()
        tool.name = "navigate_page"
        tool.description = "Go to a URL"
        tool.inputSchema = {"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"]}

        result = _convert_mcp_tools_to_openai([tool])
        assert len(result) == 1
        assert result[0]["type"] == "function"
        assert result[0]["function"]["name"] == "navigate_page"
        assert result[0]["function"]["description"] == "Go to a URL"

    def test_empty_description_defaults_to_empty_string(self):
        tool = MagicMock()
        tool.name = "my_tool"
        tool.description = None
        tool.inputSchema = {"type": "object", "properties": {}}

        result = _convert_mcp_tools_to_openai([tool])
        assert result[0]["function"]["description"] == ""

    def test_none_input_schema_defaults_to_empty_object(self):
        tool = MagicMock()
        tool.name = "my_tool"
        tool.description = "desc"
        tool.inputSchema = None

        result = _convert_mcp_tools_to_openai([tool])
        assert result[0]["function"]["parameters"] == {"type": "object", "properties": {}}

    def test_multiple_tools(self):
        tools = []
        for name in ["navigate", "click", "type_text"]:
            t = MagicMock()
            t.name = name
            t.description = f"desc for {name}"
            t.inputSchema = {"type": "object", "properties": {}}
            tools.append(t)

        result = _convert_mcp_tools_to_openai(tools)
        assert len(result) == 3
        names = [r["function"]["name"] for r in result]
        assert names == ["navigate", "click", "type_text"]


class TestExtractToolResultText:
    def test_single_text_content(self):
        result = MagicMock()
        item = MagicMock()
        item.text = "page loaded successfully"
        result.content = [item]

        assert _extract_tool_result_text(result) == "page loaded successfully"

    def test_multiple_text_content_joined_with_newline(self):
        result = MagicMock()
        i1 = MagicMock()
        i1.text = "line one"
        i2 = MagicMock()
        i2.text = "line two"
        result.content = [i1, i2]

        assert _extract_tool_result_text(result) == "line one\nline two"

    def test_non_text_content_falls_back_to_str(self):
        result = MagicMock()
        item = MagicMock()
        item.text = None
        result.content = [item]

        output = _extract_tool_result_text(result)
        assert output == str(item)

    def test_empty_content_returns_str_of_result(self):
        result = MagicMock()
        result.content = []
        output = _extract_tool_result_text(result)
        assert output == str(result)

    def test_no_content_attribute_returns_str(self):
        result = "plain string result"
        assert _extract_tool_result_text(result) == "plain string result"


class TestCaptureImagesFromHtml:
    async def test_extracts_and_fetches_images(self):
        html = '<html><body><img src="/img1.png"><img src="https://example.com/img2.jpg"></body></html>'
        with patch("app.client.fetch_image_bytes", new_callable=AsyncMock) as mock_fetch:
            mock_fetch.return_value = b"\x89PNG"
            result = await _capture_images_from_html(html, "https://example.com")
            assert "https://example.com/img1.png" in result
            assert "https://example.com/img2.jpg" in result

    async def test_skips_failed_fetches(self):
        html = '<img src="/ok.png"><img src="/fail.png">'
        with patch("app.client.fetch_image_bytes", new_callable=AsyncMock) as mock_fetch:
            async def side_effect(url, base):
                if "ok" in url:
                    return b"\x89PNG"
                return None
            mock_fetch.side_effect = side_effect
            result = await _capture_images_from_html(html, "https://example.com")
            assert "https://example.com/ok.png" in result
            assert "https://example.com/fail.png" not in result

    async def test_skips_images_without_src(self):
        html = '<img src=""><img><img src="/valid.png">'
        with patch("app.client.fetch_image_bytes", new_callable=AsyncMock) as mock_fetch:
            mock_fetch.return_value = b"\x89PNG"
            result = await _capture_images_from_html(html, "https://example.com")
            assert len(result) == 1
            assert "https://example.com/valid.png" in result