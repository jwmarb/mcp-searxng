"""Configuration for the mcp-searxng server.

Loads settings from environment variables (via .env file) and provides lazy
validation — the server always starts, unconfigured LLM roles warn at startup
and fail at use time.
"""

from __future__ import annotations

import logging
import os
from dataclasses import dataclass

from dotenv import load_dotenv
from pydantic import BaseModel, ConfigDict

logger = logging.getLogger(__name__)


class ConfigError(Exception):
    """Raised when a configuration value is missing or invalid at use time."""


class LLMRoleConfig(BaseModel):
    """Configuration for a single LLM role (Refiner, Captioner, or Agent).

    A role is "configured" when both ``base_url`` and ``model`` are non-empty.
    Unconfigured roles are allowed at startup — they warn and fail at use time.
    """

    model_config = ConfigDict(extra="forbid")

    base_url: str = ""
    api_key: str = ""
    model: str = ""

    @property
    def is_configured(self) -> bool:
        """True when base_url and model are both non-empty."""
        return bool(self.base_url.strip() and self.model.strip())


class Settings(BaseModel):
    """Top-level settings for the mcp-searxng server."""

    model_config = ConfigDict(extra="forbid")

    searxng_url: str = "http://localhost:8888"
    cache_path: str = "./cache.db"
    refiner: LLMRoleConfig = LLMRoleConfig()
    captioner: LLMRoleConfig = LLMRoleConfig()
    agent: LLMRoleConfig = LLMRoleConfig()


def load_settings() -> Settings:
    """Load settings from environment variables (and .env file).

    Returns:
        A populated :class:`Settings` instance.  Never raises on missing LLM
        config — unconfigured roles default to empty strings.
    """
    load_dotenv()

    def _role(prefix: str) -> LLMRoleConfig:
        return LLMRoleConfig(
            base_url=os.environ.get(f"{prefix}_BASE_URL", ""),
            api_key=os.environ.get(f"{prefix}_API_KEY", ""),
            model=os.environ.get(f"{prefix}_MODEL", ""),
        )

    return Settings(
        searxng_url=os.environ.get("SEARXNG_URL", "http://localhost:8888"),
        cache_path=os.environ.get("SEARXNG_CACHE_PATH", "./cache.db"),
        refiner=_role("REFINER"),
        captioner=_role("CAPTIONER"),
        agent=_role("AGENT"),
    )


def warn_unconfigured(settings: Settings) -> None:
    """Log a WARNING for each unconfigured LLM role.

    Called at server startup.  Does not raise.
    """
    role_names: list[tuple[str, LLMRoleConfig]] = [
        ("Refiner", settings.refiner),
        ("Captioner", settings.captioner),
        ("Fallback-fetch Agent", settings.agent),
    ]
    for name, cfg in role_names:
        if not cfg.is_configured:
            logger.warning(
                "%s LLM not configured (set %s_BASE_URL and %s_MODEL). "
                "This role will fail at use time.",
                name,
                name.split()[0].upper(),
                name.split()[0].upper(),
            )


def get_settings() -> Settings:
    """Return the module-level settings singleton."""
    return _settings


def is_role_configured(role: LLMRoleConfig) -> bool:
    """True when the role has both base_url and model set."""
    return role.is_configured


# Module-level singleton — loaded once on import.
_settings = load_settings()
