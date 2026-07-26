# magic-wallstreetcn-rs

Read-only, metadata-only Provider for the one public first-party WallstreetCN
RSS feed:

`https://dedicated.wallstreetcn.com/rss.xml`

The crate maps article title, decimal ID, exact canonical URL, publication
time, publisher, language, topic, and source evidence. `summary` and `content`
are always absent. It does not fetch article pages, fast-news/private APIs,
login state, cookies, descriptions, bodies, or media, and it implements no
cache, storage, index, or inferred instrument identity.

## Bounds

- exact request and final URL; redirects disabled;
- accepted base MIME:
  `application/rss+xml`, `application/xml`, `text/xml`, or the currently
  observed mislabel `text/html`;
- a complete RSS 2.0/channel/item document is mandatory for every MIME;
- 2 MiB response, 100 source items, 1–50 returned items;
- timeout 1–60 seconds, default 10;
- clone-shared one-second request-start pacing held through the response;
- complete-feed validation before caller-limit truncation.

`NewsProvider::global_news` is admitted after the 2026-07-26 release live and
two-request serial-load probes passed. `instrument_news` remains typed
`Unsupported`; news text is never promoted to a security identity.

## Probes

```bash
cargo run -p magic-wallstreetcn-rs --example live_probe --release --locked --offline

MAGIC_WALLSTREETCN_LIMIT=20 MAGIC_WALLSTREETCN_MATCH='半导体' \
cargo run -p magic-wallstreetcn-rs --example live_probe --release --locked --offline

MAGIC_WALLSTREETCN_LOAD_REQUESTS=2 \
cargo run -p magic-wallstreetcn-rs --example load_probe --release --locked --offline
```

The title match is local and case-sensitive after one bounded current-feed
fetch; it is not historical search. Load requests default to two and are
limited to one through three.

Failures remain typed as `InvalidRequest`, `Transport`, `Decode`, `Protocol`,
`Unsupported`, or `Core`. Strict records use
`ProviderId::WallstreetCn`; batch provenance source is
`wallstreetcn-rss-v1`.

See [the integration contract](../../docs/integrations/wallstreetcn-rss.md)
for field, parser, deployment, rights, and admission evidence details.
