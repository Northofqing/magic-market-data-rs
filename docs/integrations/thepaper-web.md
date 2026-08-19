# The Paper native financial news

`magic-thepaper-rs` is a read-only supplemental Provider for native articles on The
Paper's finance channel. It does not use a user account, cookies, a desktop terminal, or
a paid SDK.

## Verified upstream boundary

The official finance channel is:

```text
GET https://www.thepaper.cn/channel_25951
```

The provider accepts only HTTP 200 HTML from that exact first-party URL and parses the
single embedded `<script id="__NEXT_DATA__" type="application/json">` payload. It
requires the Next.js route `/channel/[id]`, `pageProps.id == "25951"`, and an application
payload status of 200.

Only rows whose `isOutForward` and legacy `isOutForword` fields both equal `"0"` and
whose `link` is empty or null are normalized. A non-empty `link` is the stronger external
signal and excludes the row even if both forward flags incorrectly equal `"0"`. External
forwards are omitted, so their original publishers are never relabeled as The Paper.
Native canonical URLs use:

```text
https://www.thepaper.cn/newsDetail_forward_{contId}
```

`pubTimeLong` is retained verbatim as `evidence.source_at=unix-ms:<value>`.
`published_at` is RFC3339 at `+08:00` with the same millisecond precision, so
the two fields identify exactly the same instant rather than merely the same
second.

## Bounds and failure behavior

- Caller limit: 1 through 20.
- Response cap: 2 MiB.
- Redirects: disabled.
- Production pacing: client clones share one gate; request starts are at least one second
  apart.
- HTTP, content-type, embedded-JSON boundary, page/channel, status, ID, duplicate,
  timestamp, forward-flag, metadata, URL, and evidence failures remain typed errors.
- An empty eligible native batch is not a successful result.
- `instrument_news` is explicitly unsupported because the page supplies no verified
  structured security/date filter.

Each `NewsItem` retains the source `contId`, title, RFC 3339 source time, The Paper
attribution, official native detail URL, subsection and tags, `ProviderId::ThePaper`,
observation time, and batch ID. Page order is not trusted: records are normalized by
absolute source time in descending order. Text mentions are not promoted into unverified
`InstrumentId` values.

## Operations

```bash
cargo test -p magic-thepaper-rs --all-targets --locked --offline
cargo run -p magic-thepaper-rs --example live_probe --release
MAGIC_THEPAPER_LOAD_REQUESTS=2 \
  cargo run -p magic-thepaper-rs --example load_probe --release
```

The load probe defaults to two requests, accepts at most three, uses concurrency one, and
reports failures and latency percentiles.
