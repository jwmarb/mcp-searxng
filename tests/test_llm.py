from unittest.mock import AsyncMock, patch

import httpx
import pytest

from app.llm import (
    LLMError,
    _call_chat_completions,
    _guess_mime_type,
    caption_image,
    refine_draft,
)


class TestGuessMimeType:
    def test_png(self):
        assert _guess_mime_type(b"\x89PNG\r\n\x1a\n" + b"\x00" * 10) == "image/png"

    def test_jpeg(self):
        assert _guess_mime_type(b"\xff\xd8\xff" + b"\x00" * 10) == "image/jpeg"

    def test_gif(self):
        assert _guess_mime_type(b"GIF89a" + b"\x00" * 10) == "image/gif"

    def test_webp(self):
        data = b"RIFF" + b"\x00" * 4 + b"WEBP" + b"\x00" * 10
        assert _guess_mime_type(data) == "image/webp"

    def test_svg_xml(self):
        assert _guess_mime_type(b"<?xml version...") == "image/svg+xml"

    def test_svg_tag(self):
        assert _guess_mime_type(b"<svg xmlns...") == "image/svg+xml"

    def test_unknown(self):
        assert _guess_mime_type(b"random bytes") == "application/octet-stream"


class TestCallChatCompletions:
    async def test_raises_when_unconfigured(self):
        from app.config import LLMRoleConfig

        role = LLMRoleConfig()
        with pytest.raises(LLMError, match="not configured"):
            await _call_chat_completions(role, [{"role": "user", "content": "hi"}])

    async def test_success_response(self):
        from app.config import LLMRoleConfig

        role = LLMRoleConfig(base_url="http://fake:8080/v1", api_key="k", model="m")
        mock_response = {
            "choices": [{"message": {"content": "refined output"}}]
        }
        with patch("app.llm.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.post.return_value = AsyncMock(
                status_code=200,
                json=lambda: mock_response,
                raise_for_status=lambda: None,
            )
            MockClient.return_value.__aenter__.return_value = instance
            result = await _call_chat_completions(role, [{"role": "user", "content": "hi"}])
            assert result == "refined output"

    async def test_http_error_raises_llm_error(self):
        from app.config import LLMRoleConfig

        role = LLMRoleConfig(base_url="http://fake:8080/v1", api_key="k", model="m")
        with patch("app.llm.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            exc = httpx.HTTPStatusError(
                "500 Server Error",
                request=httpx.Request("POST", "http://fake"),
                response=httpx.Response(500),
            )
            instance.post.side_effect = exc
            MockClient.return_value.__aenter__.return_value = instance
            with pytest.raises(LLMError, match="HTTP 500"):
                await _call_chat_completions(role, [{"role": "user", "content": "hi"}])

    async def test_request_error_raises_llm_error(self):
        from app.config import LLMRoleConfig

        role = LLMRoleConfig(base_url="http://fake:8080/v1", api_key="k", model="m")
        with patch("app.llm.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.post.side_effect = httpx.RequestError("connection refused")
            MockClient.return_value.__aenter__.return_value = instance
            with pytest.raises(LLMError, match="Cannot reach"):
                await _call_chat_completions(role, [{"role": "user", "content": "hi"}])

    async def test_malformed_response_raises_llm_error(self):
        from app.config import LLMRoleConfig

        role = LLMRoleConfig(base_url="http://fake:8080/v1", api_key="k", model="m")
        with patch("app.llm.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.post.return_value = AsyncMock(
                status_code=200,
                json=lambda: {"choices": []},
                raise_for_status=lambda: None,
            )
            MockClient.return_value.__aenter__.return_value = instance
            with pytest.raises(LLMError, match="Malformed"):
                await _call_chat_completions(role, [{"role": "user", "content": "hi"}])

    async def test_sends_correct_headers(self):
        from app.config import LLMRoleConfig

        role = LLMRoleConfig(base_url="http://fake:8080/v1", api_key="secret", model="m")
        with patch("app.llm.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.post.return_value = AsyncMock(
                status_code=200,
                json=lambda: {"choices": [{"message": {"content": "ok"}}]},
                raise_for_status=lambda: None,
            )
            MockClient.return_value.__aenter__.return_value = instance
            await _call_chat_completions(role, [{"role": "user", "content": "hi"}])
            call_kwargs = instance.post.call_args
            headers = call_kwargs.kwargs.get("headers", call_kwargs[1].get("headers", {}))
            assert headers["Authorization"] == "Bearer secret"
            assert headers["Content-Type"] == "application/json"

    async def test_no_api_key_omits_auth_header(self):
        from app.config import LLMRoleConfig

        role = LLMRoleConfig(base_url="http://fake:8080/v1", api_key="", model="m")
        with patch("app.llm.httpx.AsyncClient") as MockClient:
            instance = AsyncMock()
            instance.post.return_value = AsyncMock(
                status_code=200,
                json=lambda: {"choices": [{"message": {"content": "ok"}}]},
                raise_for_status=lambda: None,
            )
            MockClient.return_value.__aenter__.return_value = instance
            await _call_chat_completions(role, [{"role": "user", "content": "hi"}])
            headers = instance.post.call_args.kwargs.get("headers", {})
            assert "Authorization" not in headers


class TestRefineDraft:
    async def test_calls_llm_with_system_prompt(self):
        from app.config import LLMRoleConfig, Settings

        settings = Settings(refiner=LLMRoleConfig(base_url="http://fake/v1", api_key="k", model="m"))
        with patch("app.llm.get_settings", return_value=settings):
            with patch("app.llm.httpx.AsyncClient") as MockClient:
                instance = AsyncMock()
                instance.post.return_value = AsyncMock(
                    status_code=200,
                    json=lambda: {"choices": [{"message": {"content": "# Refined\n\nContent."}}]},
                    raise_for_status=lambda: None,
                )
                MockClient.return_value.__aenter__.return_value = instance
                result = await refine_draft("# Draft\n\nRaw content.")
                assert result == "# Refined\n\nContent."

    async def test_raises_when_unconfigured(self):
        from app.config import Settings

        settings = Settings()
        with patch("app.llm.get_settings", return_value=settings):
            with pytest.raises(LLMError, match="not configured"):
                await refine_draft("draft")


class TestCaptionImage:
    async def test_calls_llm_with_image_data_url(self):
        from app.config import LLMRoleConfig, Settings

        settings = Settings(captioner=LLMRoleConfig(base_url="http://fake/v1", api_key="k", model="m"))
        image_data = b"\x89PNG\r\n\x1a\n" + b"\x00" * 100
        with patch("app.llm.get_settings", return_value=settings):
            with patch("app.llm.httpx.AsyncClient") as MockClient:
                instance = AsyncMock()
                instance.post.return_value = AsyncMock(
                    status_code=200,
                    json=lambda: {"choices": [{"message": {"content": "A cat sitting on a table"}}]},
                    raise_for_status=lambda: None,
                )
                MockClient.return_value.__aenter__.return_value = instance
                result = await caption_image(image_data)
                assert result == "A cat sitting on a table"
                call_kwargs = instance.post.call_args
                payload = call_kwargs.kwargs.get("json", call_kwargs[1].get("json", {}))
                user_content = payload["messages"][1]["content"]
                assert any(c.get("type") == "image_url" for c in user_content)

    async def test_includes_alt_text_when_provided(self):
        from app.config import LLMRoleConfig, Settings

        settings = Settings(captioner=LLMRoleConfig(base_url="http://fake/v1", api_key="k", model="m"))
        image_data = b"\x89PNG\r\n\x1a\n" + b"\x00" * 100
        with patch("app.llm.get_settings", return_value=settings):
            with patch("app.llm.httpx.AsyncClient") as MockClient:
                instance = AsyncMock()
                instance.post.return_value = AsyncMock(
                    status_code=200,
                    json=lambda: {"choices": [{"message": {"content": "caption"}}]},
                    raise_for_status=lambda: None,
                )
                MockClient.return_value.__aenter__.return_value = instance
                await caption_image(image_data, original_alt="Figure 1: Results")
                payload = instance.post.call_args.kwargs.get("json", {})
                user_content = payload["messages"][1]["content"]
                text_parts = [c for c in user_content if c.get("type") == "text"]
                assert len(text_parts) == 1
                assert "Figure 1: Results" in text_parts[0]["text"]

    async def test_raises_when_unconfigured(self):
        from app.config import Settings

        settings = Settings()
        with patch("app.llm.get_settings", return_value=settings):
            with pytest.raises(LLMError, match="not configured"):
                await caption_image(b"\x89PNG")