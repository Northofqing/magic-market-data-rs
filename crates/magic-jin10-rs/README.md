# magic-jin10-rs

Read-only adapter for the public Jin10 7x24 financial flash stream.

- Implements `NewsProvider::global_news`; instrument/date filtering is explicitly
  unsupported.
- Calls the first-party
  `https://flash-api.jin10.com/get_flash_list?channel=-8200&vip=1` JSON endpoint with
  headers used by the official web client.
- Admits public type-0 flashes and type-2 linked articles only. Locked VIP placeholders
  are omitted without requesting protected details.
- Requires HTTP 200 JSON from the exact official HTTPS host, follows zero redirects, and
  caps responses at 2 MiB.
- Validates the envelope, 20-row bound, IDs, duplicates, timestamps, public content,
  tags, canonical URLs, and Core evidence.
- Client clones share a request gate held through the complete response read; production
  request starts are at least one second apart.
- The bounded load probe is capped at three sequential requests and reports errors, RPS,
  and p50/p95/p99/max latency.

```bash
cargo run -p magic-jin10-rs --example live_probe --release
MAGIC_JIN10_LOAD_REQUESTS=2 \
  cargo run -p magic-jin10-rs --example load_probe --release
```
