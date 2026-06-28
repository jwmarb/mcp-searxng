# Two-layer cache with parallel Refiner+Captioner and splice-on-read

On a Page-cache miss, the Refiner (text-to-text) and Captioner (vision,
image-to-text) run in parallel. The Refiner cleans the Draft and emits
positional image markers (`<!--IMG:i-->`); the Captioner describes each
image independently. Captions are spliced into the markers after both
complete. The Page cache stores the Refiner's marker'd output + a caption
map + the set of missing indices — not the final spliced Markdown. The
spliced Markdown is rendered from these components on every read.

## Considered options

- **Sequential: Refiner first, then Captioner, captions spliced into final
  Markdown.** Rejected: serializes two independent LLM roles; the Refiner
  can't start until the Captioner finishes (or vice versa), wasting the
  parallelism that local-LLM throughput allows.

- **Store the final spliced Markdown in the Page cache.** Rejected:
  breaks the incomplete-page retry design (Q10/Reading D). If captions are
  missing, the markers are gone (replaced by captions on first splice), so
  missing captions can't be retried without re-running the Refiner.

- **Store both final Markdown (for fast hits) and marker'd output + caption
  map (for retries).** Rejected: double storage for no functional benefit.
  The splice step is cheap (string replacement, microseconds); optimizing
  it away isn't worth duplicating the Refiner output in the cache. Two
  sources of truth that can drift on caption correction.

- **Parallel Refiner+Captioner, store marker'd output + caption map,
  splice on read (chosen).** One source of truth. Missing captions can be
  retried by reusing the cached marker'd output and re-captioning only the
  missing indices. Per-caption correction is possible by editing the
  caption map. Splice cost is negligible.

## Consequences

- Every cache hit does a splice (string replacement of N markers with N
  captions). N is typically <20. Cost is microseconds, well below LLM and
  even markdownify costs.
- The Page cache schema has three fields per entry: `refiner_output`,
  `captions` (a JSON map of index→text), `missing` (a list of indices).
  The "complete" state is `missing == []`.
- Caption correction (e.g. after upgrading the vision model) is possible by
  deleting the Image-cache entry for an image's hash and removing that
  index from the Page cache's caption map and adding it to `missing`. The
  next fetch re-captions and re-splices.
