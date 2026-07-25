# Findings: Yonhap Chinese RSS News Provider

## Project Context

- `NewsProvider` exposes `instrument_news(request)` and
  `global_news(limit)`.
- `NewsItem` already represents item ID, title, optional summary/body,
  publisher, canonical URL, published time, instruments, topics, language, and
  record evidence.
- `GlobalNewsRouter` is provider-neutral and rejects batches that exceed the
  requested limit or contain duplicate item IDs. The failover chain also
  rejects record/provider provenance mismatches.
- Existing first-class news crates use exact HTTPS allowlists, zero redirects,
  bounded bodies, shared request gates, deterministic fixtures, live/load
  probes, capability tests, integration docs, and compliance registration.
- Adding a Provider requires a `ProviderId` variant, core identity coverage,
  workspace membership, a new crate, Router identity coverage, docs, and
  compliance registration.

## Official Source

- Yonhap's simplified-Chinese RSS guide publishes exactly seven feeds:
  rolling, politics, economy, society, culture/sports, North Korea, and
  China–Korea relations.
- The guide describes RSS as a way for readers to receive Yonhap's latest
  messages without visiting the website.
- The Chinese and English copyright pages prohibit unauthorized copying,
  storage, transmission, display, sale, modification, and information-service
  redistribution.
- Therefore the Provider may act as a bounded RSS reader and expose source
  metadata, but must ignore descriptions and never fetch article pages or
  persist bodies.

## Current Network Evidence

- Browser-backed source inspection can read Yonhap article and RSS-guide pages.
- Direct system `curl` to `https://cn.yna.co.kr/RSS/news.xml` fails during TLS
  initialization with `SSL_ERROR_SYSCALL`, including outside the sandbox.
- This does not prove that the Rust TLS transport will fail, but it prevents a
  capability claim until the production client completes a bounded live probe.

## Official References

- RSS guide: <https://cn.yna.co.kr/channel/rss>
- Chinese terms: <https://cn.yna.co.kr/aboutus/copyright>
- Reference article:
  <https://cn.yna.co.kr/view/ACK20260629002400881>
- English initiating article:
  <https://en.yna.co.kr/view/AEN20260725001153315>
