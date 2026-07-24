# Jin10 public financial flashes

`magic-jin10-rs` is a read-only supplemental Provider for the public Jin10 7x24
financial-flash stream. It does not use a user account, cookies, a desktop terminal, or a
paid SDK.

## Verified upstream boundary

The official Jin10 web bundle calls:

```text
GET https://flash-api.jin10.com/get_flash_list?channel=-8200&vip=1
x-app-id: bVBF4FyRTn5NJF5n
x-version: 1.0.0
```

The provider sends the official public app/version identifiers, `Origin`, `Referer`,
`Accept`, and its own `User-Agent`. The `vip=1` query field is part of the public web
request shape; it does not grant access. Rows whose public payload declares
`data.lock == true` are omitted, and the provider never requests or decrypts a protected
detail.

Only public type-0 flashes and type-2 linked articles are normalized. Economic-calendar
and other non-news row types are outside `NewsProvider::global_news`.

## Bounds and failure behavior

- Caller limit: 1 through 20.
- Response cap: 2 MiB.
- Redirects: disabled.
- Production pacing: client clones share one gate; request starts are at least one second
  apart.
- HTTP, content-type, envelope, ID, duplicate, timestamp, public-content, URL, tag, and
  evidence failures remain typed errors.
- An empty eligible public batch is not a successful result.
- `instrument_news` is explicitly unsupported because the public stream supplies no
  verified structured security filter.

Each `NewsItem` retains the source row ID, RFC 3339 source time, content, attribution,
official detail/article URL, language, topics, `ProviderId::Jin10`, observation time, and
batch ID. Text mentions are not promoted into unverified `InstrumentId` values.

## Operations

```bash
cargo test -p magic-jin10-rs --all-targets --locked --offline
cargo run -p magic-jin10-rs --example live_probe --release
MAGIC_JIN10_LOAD_REQUESTS=2 \
  cargo run -p magic-jin10-rs --example load_probe --release
```

The load probe defaults to two requests, accepts at most three, uses concurrency one, and
reports failures and latency percentiles.
