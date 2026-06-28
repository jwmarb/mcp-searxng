# Three independent LLM endpoints with graceful degradation

Each of the three LLM roles (Refiner, Captioner, Fallback-fetch agent) has
its own `*_BASE_URL`, `*_API_KEY`, and `*_MODEL` in `.env`. There is no
shared default model or endpoint. Each role fails independently and
gracefully; the server always starts regardless of LLM configuration.

## Considered options

- **One endpoint, one model for all roles.** Rejected: the three roles have
  different ideal model profiles (text-fast for Refiner, vision for
  Captioner, tool-use-capable for Agent). Forcing one model wastes the
  ability to differentiate. Power users self-hosting different models for
  different purposes lose granularity.

- **One endpoint, three model names (with default).** A default
  `LLM_MODEL` for all roles, overridable per role. Rejected: the user
  explicitly chose full customization. Defaults add a "what if the user
  forgets to set one" path that doesn't match the "power user configures
  deliberately" audience.

- **Three independent endpoints, no shared default (chosen).** Nine env
  vars (three per role). Power users configure each role deliberately. No
  silent fallback to a default that might be the wrong model for the job.

## Consequences

- `web_search` works with zero LLM configuration. The server always
  starts.
- The Refiner being unavailable returns the unrefined Draft (not cached;
  self-heals when the Refiner returns). The Captioner being unavailable
  caches the page as incomplete (all images missing; self-heals). The
  Agent being unavailable hard-fails that URL (no fallback possible — no
  HTML exists).
- A user who only wants search doesn't configure any LLM. A user who
  wants page reading configures the Refiner. A user who wants image
  captions adds the Captioner. A user who wants Cloudflare-bypass adds the
  Agent. Capabilities compose incrementally.
