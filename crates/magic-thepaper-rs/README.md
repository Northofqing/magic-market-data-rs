# magic-thepaper-rs

Read-only adapter for native articles on The Paper's finance channel.

- Implements `NewsProvider::global_news`; instrument/date filtering is explicitly
  unsupported.
- Reads the first-party `https://www.thepaper.cn/channel_25951` page and its embedded
  `__NEXT_DATA__` JSON.
- Admits only rows whose two forwarding flags identify a native article and whose
  external link is empty. External forwards are omitted instead of being attributed to
  The Paper.
- Requires HTTP 200 HTML from the exact official HTTPS URL, follows zero redirects, and
  caps responses at 2 MiB.
- Validates the page/channel identity, payload status, 20-row bound, IDs, duplicates,
  timestamps, native flags, section/tag metadata, canonical URLs, and Core evidence.
- Client clones share a request gate held through the complete response read; production
  request starts are at least one second apart.
- The bounded load probe is capped at three sequential requests and reports errors, RPS,
  and p50/p95/p99/max latency.

```bash
cargo run -p magic-thepaper-rs --example live_probe --release
MAGIC_THEPAPER_LOAD_REQUESTS=2 \
  cargo run -p magic-thepaper-rs --example load_probe --release
```
