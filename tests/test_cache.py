import pytest

from app.cache import (
    ImageCache,
    PageCache,
    PageCacheEntry,
    hash_draft,
    hash_image_bytes,
    splice_captions,
)


class TestHashFunctions:
    def test_hash_draft_deterministic(self):
        h1 = hash_draft("hello world")
        h2 = hash_draft("hello world")
        assert h1 == h2
        assert len(h1) == 64

    def test_hash_draft_different_input(self):
        h1 = hash_draft("hello")
        h2 = hash_draft("world")
        assert h1 != h2

    def test_hash_draft_unicode(self):
        h = hash_draft("hello \u2603 world")
        assert len(h) == 64

    def test_hash_image_bytes_deterministic(self):
        data = b"\x89PNG\r\n\x1a\n" + b"\x00" * 10
        h1 = hash_image_bytes(data)
        h2 = hash_image_bytes(data)
        assert h1 == h2
        assert len(h1) == 64

    def test_hash_image_bytes_different(self):
        h1 = hash_image_bytes(b"\x89PNG\r\n\x1a\n")
        h2 = hash_image_bytes(b"\xff\xd8\xff")
        assert h1 != h2


class TestSpliceCaptions:
    def test_splice_all_captions_present(self):
        refiner_out = "Hello <!--IMG:0--> world <!--IMG:1-->"
        captions = {0: "a cat", 1: "a dog"}
        image_map = {0: {"url": "/cat.png", "alt": ""}, 1: {"url": "/dog.png", "alt": ""}}
        result = splice_captions(refiner_out, captions, image_map)
        assert "![a cat](/cat.png)" in result
        assert "![a dog](/dog.png)" in result
        assert "<!--IMG:" not in result

    def test_splice_missing_captions_produce_empty_alt(self):
        refiner_out = "Text <!--IMG:0-->"
        captions = {}
        image_map = {0: {"url": "/img.png", "alt": ""}}
        result = splice_captions(refiner_out, captions, image_map)
        assert "![](/img.png)" in result

    def test_splice_escapes_brackets_in_caption(self):
        refiner_out = "<!--IMG:0-->"
        captions = {0: "chart [Q1] vs [Q2]"}
        image_map = {0: {"url": "/chart.png", "alt": ""}}
        result = splice_captions(refiner_out, captions, image_map)
        assert "![chart \\[Q1\\] vs \\[Q2\\]](/chart.png)" in result

    def test_splice_no_markers_returns_unchanged(self):
        refiner_out = "Just plain text."
        result = splice_captions(refiner_out, {}, {})
        assert result == "Just plain text."

    def test_splice_unknown_index_uses_empty_url(self):
        refiner_out = "<!--IMG:99-->"
        captions = {99: "something"}
        image_map = {}
        result = splice_captions(refiner_out, captions, image_map)
        assert "![something]() " in result or "![something]()" in result


class TestPageCacheEntry:
    def test_frozen(self):
        entry = PageCacheEntry(
            hash="abc",
            refiner_output="text",
            captions={0: "cap"},
            missing=[1],
            created_at="2024-01-01",
        )
        with pytest.raises(Exception):
            entry.hash = "xyz"

    def test_fields(self):
        entry = PageCacheEntry(
            hash="abc123",
            refiner_output="refined <!--IMG:0-->",
            captions={0: "a cat"},
            missing=[1, 2],
            created_at="2024-06-01T00:00:00",
        )
        assert entry.hash == "abc123"
        assert entry.captions == {0: "a cat"}
        assert entry.missing == [1, 2]


class TestPageCache:
    @pytest.fixture(autouse=True)
    def _db(self, temp_db_path):
        self.cache = PageCache(temp_db_path)

    async def test_init_creates_table(self):
        await self.cache.init()

    async def test_store_and_get(self):
        await self.cache.init()
        await self.cache.store("hash1", "refined output", {0: "cat"}, [1])
        entry = await self.cache.get("hash1")
        assert entry is not None
        assert entry.hash == "hash1"
        assert entry.refiner_output == "refined output"
        assert entry.captions == {0: "cat"}
        assert entry.missing == [1]

    async def test_get_miss_returns_none(self):
        await self.cache.init()
        assert await self.cache.get("nonexistent") is None

    async def test_store_overwrites(self):
        await self.cache.init()
        await self.cache.store("hash1", "v1", {}, [0, 1])
        await self.cache.store("hash1", "v2", {0: "dog"}, [1])
        entry = await self.cache.get("hash1")
        assert entry.refiner_output == "v2"
        assert entry.captions == {0: "dog"}
        assert entry.missing == [1]

    async def test_update_captions(self):
        await self.cache.init()
        await self.cache.store("hash1", "output", {0: "cat"}, [1, 2])
        await self.cache.update_captions("hash1", {0: "cat", 1: "dog"}, [2])
        entry = await self.cache.get("hash1")
        assert entry.captions == {0: "cat", 1: "dog"}
        assert entry.missing == [2]

    async def test_multiple_entries(self):
        await self.cache.init()
        await self.cache.store("h1", "out1", {}, [])
        await self.cache.store("h2", "out2", {}, [])
        e1 = await self.cache.get("h1")
        e2 = await self.cache.get("h2")
        assert e1.refiner_output == "out1"
        assert e2.refiner_output == "out2"


class TestImageCache:
    @pytest.fixture(autouse=True)
    def _db(self, temp_db_path):
        self.cache = ImageCache(temp_db_path)

    async def test_init_creates_table(self):
        await self.cache.init()

    async def test_store_and_get(self):
        await self.cache.init()
        await self.cache.store("img_hash_1", "a beautiful sunset")
        result = await self.cache.get("img_hash_1")
        assert result == "a beautiful sunset"

    async def test_get_miss_returns_none(self):
        await self.cache.init()
        assert await self.cache.get("nope") is None

    async def test_store_overwrites(self):
        await self.cache.init()
        await self.cache.store("h1", "caption v1")
        await self.cache.store("h1", "caption v2")
        assert await self.cache.get("h1") == "caption v2"

    async def test_multiple_entries(self):
        await self.cache.init()
        await self.cache.store("h1", "cat")
        await self.cache.store("h2", "dog")
        assert await self.cache.get("h1") == "cat"
        assert await self.cache.get("h2") == "dog"