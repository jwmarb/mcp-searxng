"""Pydantic models for SearXNG API responses."""

from pydantic import BaseModel


class SearchResult(BaseModel):
    """A single search result from SearXNG."""

    url: str
    title: str
    content: str


class InfoboxUrl(BaseModel):
    """A URL reference within an infobox."""

    title: str
    url: str


class Infobox(BaseModel):
    """An infobox result from SearXNG."""

    infobox: str
    id: str
    content: str
    urls: list[InfoboxUrl]


class Response(BaseModel):
    """Full SearXNG search response."""

    query: str
    number_of_results: int
    results: list[SearchResult]
    infoboxes: list[Infobox]