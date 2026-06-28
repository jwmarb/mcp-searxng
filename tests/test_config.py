import os
from unittest.mock import patch

import pytest

from app.config import LLMRoleConfig, Settings, load_settings, warn_unconfigured


class TestLLMRoleConfig:
    def test_is_configured_true(self):
        cfg = LLMRoleConfig(base_url="http://localhost:8080/v1", api_key="key", model="gpt-4")
        assert cfg.is_configured is True

    def test_is_configured_no_base_url(self):
        cfg = LLMRoleConfig(api_key="key", model="gpt-4")
        assert cfg.is_configured is False

    def test_is_configured_no_model(self):
        cfg = LLMRoleConfig(base_url="http://localhost:8080/v1", api_key="key")
        assert cfg.is_configured is False

    def test_is_configured_whitespace_only(self):
        cfg = LLMRoleConfig(base_url="   ", model="  ")
        assert cfg.is_configured is False

    def test_extra_forbidden(self):
        with pytest.raises(Exception):
            LLMRoleConfig(base_url="x", model="x", extra_field="bad")  # type: ignore[arg-type]


class TestSettings:
    def test_defaults(self):
        s = Settings()
        assert s.searxng_url == "http://localhost:8888"
        assert s.cache_path == "./cache.db"
        assert s.refiner.is_configured is False
        assert s.captioner.is_configured is False
        assert s.agent.is_configured is False

    def test_extra_forbidden(self):
        with pytest.raises(Exception):
            Settings(extra_field="bad")  # type: ignore[arg-type]


class TestLoadSettings:
    def test_loads_from_env(self):
        env = {
            "SEARXNG_URL": "http://test:9999",
            "SEARXNG_CACHE_PATH": "/tmp/test.db",
            "REFINER_BASE_URL": "http://refiner:8080/v1",
            "REFINER_API_KEY": "ref-key",
            "REFINER_MODEL": "ref-model",
            "CAPTIONER_BASE_URL": "http://captioner:8080/v1",
            "CAPTIONER_API_KEY": "cap-key",
            "CAPTIONER_MODEL": "cap-model",
            "AGENT_BASE_URL": "http://agent:8080/v1",
            "AGENT_API_KEY": "agt-key",
            "AGENT_MODEL": "agt-model",
        }
        with patch.dict(os.environ, env, clear=False):
            with patch("app.config.load_dotenv"):
                s = load_settings()
        assert s.searxng_url == "http://test:9999"
        assert s.cache_path == "/tmp/test.db"
        assert s.refiner.is_configured is True
        assert s.refiner.base_url == "http://refiner:8080/v1"
        assert s.captioner.is_configured is True
        assert s.agent.is_configured is True

    def test_defaults_when_no_env(self):
        keys = [k for k in os.environ if k.startswith(("SEARXNG_", "REFINER_", "CAPTIONER_", "AGENT_"))]
        saved = {k: os.environ.pop(k) for k in keys}
        try:
            with patch("app.config.load_dotenv"):
                s = load_settings()
        finally:
            os.environ.update(saved)

        assert s.searxng_url == "http://localhost:8888"
        assert s.cache_path == "./cache.db"
        assert s.refiner.is_configured is False

    def test_partial_config(self):
        keys = [k for k in os.environ if k.startswith(("REFINER_", "CAPTIONER_", "AGENT_"))]
        saved = {k: os.environ.pop(k) for k in keys}
        try:
            os.environ["REFINER_BASE_URL"] = "http://refiner:8080/v1"
            os.environ["REFINER_MODEL"] = "ref-model"
            with patch("app.config.load_dotenv"):
                s = load_settings()
        finally:
            os.environ.update(saved)
        assert s.refiner.is_configured is True
        assert s.captioner.is_configured is False
        assert s.agent.is_configured is False


class TestWarnUnconfigured:
    def test_warns_on_unconfigured_roles(self, caplog):
        caplog.set_level(0)
        s = Settings()
        warn_unconfigured(s)
        assert "Refiner" in caplog.text
        assert "Captioner" in caplog.text
        assert "Fallback-fetch Agent" in caplog.text

    def test_no_warning_when_all_configured(self, caplog):
        caplog.set_level(0)
        s = Settings(
            refiner=LLMRoleConfig(base_url="http://x", model="m"),
            captioner=LLMRoleConfig(base_url="http://x", model="m"),
            agent=LLMRoleConfig(base_url="http://x", model="m"),
        )
        warn_unconfigured(s)
        assert "not configured" not in caplog.text