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
detail. A locked row is conclusively private from `lock=true`; the source may omit
`vip_level` on that row and this does not invalidate the surrounding public batch.

Only public type-0 flashes and type-2 linked articles are normalized by
`NewsProvider::global_news`. They must belong to at least one source news
channel 1, 2 or 3; source rows belonging only to channel 5 are promotional
slots and are omitted. Missing, empty or non-integral channel arrays remain
protocol failures. Public unlocked type-1 economic rows can be inspected
separately through the diagnostic `EconomicCalendarProvider`; the two contracts
are never mixed.

## Economic-calendar diagnostic

Jin10 ended its free calendar and associated API embedding service on
2025-12-01, and the current calendar page requires an authenticated session.
The remaining public flash endpoint is only a rolling latest-item window. It
cannot prove a complete calendar result for a requested date range, so
`ECONOMIC_CALENDAR_ADMITTED` is `false` and production routing never selects it.
The parser remains reachable only through explicit unadmitted diagnostic access.

The calendar adapter preserves the source event/indicator identity, country, name,
period, scheduled and released times, previous/consensus/actual/revised values, unit,
importance and impact direction. A source value of numeric zero remains the text value
`"0"` and is never converted to absence. Locked rows, duplicate identities, missing
required fields, malformed timestamps, importance outside the admitted range or an
oversized/empty eligible batch fail explicitly. Diagnostic requests accept 1
through 20 rows and may apply an exact source country filter. An empty eligible
flash window is a typed failure, never a verified-empty calendar.

## Bounds and failure behavior

- Caller limit: 1 through 20.
- Source window: at most 21 rows. The public endpoint was observed briefly returning 21
  during a rolling update before returning to its normal 20; 22 or more remains a
  protocol failure.
- Response cap: 2 MiB.
- Redirects: disabled.
- Production pacing: client clones share one gate; request starts are at least one second
  apart.
- HTTP, content-type, envelope, ID, duplicate, timestamp, public-content, URL, tag, and
  evidence failures remain typed errors.
- An empty eligible public batch is not a successful result.
- `instrument_news` is explicitly unsupported because the public stream supplies no
  verified structured security filter.

Each `NewsItem` retains the source row ID, content, attribution, official detail/article
URL, language, topics, `ProviderId::Jin10`, observation time, and batch ID. `published_at`
is normalized to RFC3339, while evidence preserves the Provider's original
`YYYY-MM-DD HH:MM:SS` string. The previous persistent `invalid_evidence` came from
requiring a redundant `vip_level` on already locked rows; no other source is used to
fill the field. Text mentions are not promoted into unverified `InstrumentId` values.

## Operations

```bash
cargo test -p magic-jin10-rs --all-targets --locked --offline
cargo run -p magic-jin10-rs --example live_probe --release
MAGIC_JIN10_LIVE_INCLUDE_CALENDAR=1 \
  cargo run -p magic-jin10-rs --example live_probe --release
MAGIC_JIN10_LOAD_REQUESTS=2 \
  cargo run -p magic-jin10-rs --example load_probe --release
```

The default live probe validates public news. Set
`MAGIC_JIN10_LIVE_INCLUDE_CALENDAR=1` to run the explicit unadmitted diagnostic
and require a current economic release; this mode can correctly fail when the
rolling public window contains no eligible type-1 row. It is not admission
evidence for a complete calendar. The load probe defaults to two requests,
accepts at most three, uses concurrency one, and reports failures and latency
percentiles.
