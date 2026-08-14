# magic-iwencai-rs

Read-only iWencai SkillHub semantic-search adapter.

- Requires an explicitly supplied API key; no browser cookies or desktop-session state is read.
- Uses `MAGIC_IWENCAI_API_KEY` (`IWENCAI_API_KEY` is a compatibility alias).
- Sends the required `Authorization` and `X-Claw-*` headers to the official
  `https://openapi.iwencai.com/v1/comprehensive/search` endpoint.
- Maps only response fields verified by deterministic fixtures and preserves the
  highest-score segment for duplicate document IDs.
- Returns a typed `Authentication` error for missing keys, HTTP 401/403, and API-level key rejection.
- Requires JSON for successful responses and caps search at 50 records and 4 MiB.
- Captures `observed_at` only after the complete response has been received.
- Clones share a serial request gate held through the complete response read;
  production request starts are at least one second apart.
- `semantic_search` is admitted for the bounded `Report` query slice after the
  2026-08-14 authorized live/load evidence; runtime calls still require a valid key.
- The bounded load probe is capped at three requests and reports errors, RPS,
  and p50/p95/p99/max latency.

```bash
MAGIC_IWENCAI_API_KEY=... cargo run -p magic-iwencai-rs --example live_probe --release
MAGIC_IWENCAI_API_KEY=... MAGIC_IWENCAI_LOAD_REQUESTS=2 \
  cargo run -p magic-iwencai-rs --example load_probe --release
```
