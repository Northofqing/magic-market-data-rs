# magic-cls-rs

Read-only 财联社电报 adapter for the signed
`https://www.cls.cn/v1/roll/get_roll_list` endpoint.

- Implements `NewsProvider::global_news`; instrument-filtered news is explicitly unsupported.
- Computes the required `md5(sha1(sorted-query))` signature locally; no API key or desktop session is used.
- Requires a successful JSON response from the official HTTPS host, follows zero
  redirects, and caps responses at 2 MiB.
- Validates `errno == 0`, record count, identifiers, source time, canonical URL, and Core evidence.
- Present-but-malformed stock/topic metadata is a protocol error; it is never
  silently discarded from an otherwise complete batch.
- Client clones share a request gate held through the complete response read;
  production request starts are at least one second apart.
- The bounded load probe is capped at three requests and reports errors, RPS,
  and p50/p95/p99/max latency.

```bash
cargo run -p magic-cls-rs --example live_probe --release
MAGIC_CLS_LOAD_REQUESTS=2 cargo run -p magic-cls-rs --example load_probe --release
```
