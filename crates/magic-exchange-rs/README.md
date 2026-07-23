# magic-exchange-rs

Read-only official-source adapters for the Shanghai Stock Exchange, Shenzhen
Stock Exchange and, in future slices, HKEX. The current admitted capability is
instrument-scoped official announcements from SSE and SZSE. `HkexClient`
currently exposes only its provider identity and advertises no data family.

## Admitted endpoints

| Client | HTTPS endpoint | Provider | Capability |
| --- | --- | --- | --- |
| `SseClient` | `query.sse.com.cn/security/stock/queryCompanyBulletin.do` | `ProviderId::Sse` | announcements |
| `SzseClient` | `www.szse.cn/api/disc/announcement/annList` | `ProviderId::Szse` | announcements |
| `HkexClient` | none | `ProviderId::Hkex` | none |

Both announcement clients require an equity on the matching exchange, a caller
limit of at most 500 and optional valid Core date bounds. They fetch fixed
50-row remote pages and truncate only after complete pages have been
validated. A strict batch rejects missing or mismatched source security/date,
invalid official URLs, duplicate IDs, changing totals, skipped/short pages,
schema drift and empty results.

The transport permits only each client's exact credential-free HTTPS host and
path on port 443. Redirects, unexpected final URLs, wrong content types,
responses over 8 MiB and timeouts outside `1..=60` seconds are rejected.
Cloned clients share a serial gate, retain it through the complete response
read and start production requests at least one second apart.

## Probes

```bash
RUSTUP_TOOLCHAIN=1.83.0 \
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline

MAGIC_EXCHANGE_LOAD_REQUESTS=4 \
MAGIC_EXCHANGE_LOAD_CONCURRENCY=1 \
MAGIC_EXCHANGE_LOAD_PACING_MS=1000 \
RUSTUP_TOOLCHAIN=1.83.0 \
cargo run -p magic-exchange-rs --example load_probe --release --locked --offline
```

Defaults are 华电辽能 `600396.SH` and 五粮液 `000858.SZ`. Override them with
`MAGIC_EXCHANGE_SSE_CODE` and `MAGIC_EXCHANGE_SZSE_CODE`; the live output limit
is controlled by `MAGIC_EXCHANGE_LIVE_LIMIT` in `1..=20`.

The 2026-07-23 acceptance run returned three real records from each exchange.
The final serial load run passed 4/4 attempts with zero failures, 0.9294
attempts/s, P50 1082 ms, P95/P99/max 1214 ms and a 1003 ms minimum attempt
start gap. These are high-level announcement-attempt metrics, not HTTP request
throughput: one attempt can fetch more than one page. Provider calls are timed
before records are printed, so console I/O is excluded from these metrics.

SZSE's sampled detail page returned HTTP 200 and its sampled PDF returned
`application/pdf`. The SSE records carry official PDF URLs, but the sampled
HEAD request was answered by CDN bot HTML; this crate therefore claims URL
metadata only, not SSE PDF-download acceptance.

## Deterministic gates

```bash
RUSTUP_TOOLCHAIN=1.83.0 cargo test -p magic-exchange-rs --all-targets --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo clippy -p magic-exchange-rs --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS="-D warnings" RUSTUP_TOOLCHAIN=1.83.0 cargo doc -p magic-exchange-rs --no-deps --locked --offline
```

No client reads cookies, account state or desktop-terminal traffic. There is no
obsolete-TLS or plaintext fallback.
