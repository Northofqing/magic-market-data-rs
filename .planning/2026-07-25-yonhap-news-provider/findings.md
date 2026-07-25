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

## Implementation Mapping

- No XML or general date/time parser is present in the current lockfile.
- `quick-xml` 0.41.0 is the current documented streaming release and exposes
  explicit `Event::DocType` and `Event::GeneralRef` variants, allowing the
  Provider to reject DTD/entity constructs before mapping.
- `time` 0.3.54 provides checked RFC 2822 parsing and explicit-offset
  formatting behind `parsing` and `formatting`; `std` is enabled explicitly
  for the Provider runtime. Using it avoids a hand-written calendar/timezone
  parser.
- The Provider manifest should pin `quick-xml = "=0.41.0"` with default
  features disabled and `time = "=0.3.54"` with `std`, `parsing`, and
  `formatting`, alongside the workspace's existing exact `ureq` TLS version.
- Release registration touches root workspace membership, Core Provider
  identity tests, Router fixture identity, `tools/compliance/check.sh`,
  `tools/release/package.sh`, root README, deployment docs, business rules,
  and a dedicated integration document.
- Strict coverage automatically discovers every production `.rs` source from
  workspace manifests, so the new crate must add enough deterministic
  behavior tests to keep the existing 80% overall gate green.
- The detailed plan uses a hard two-state admission rule: two consecutive
  successful production-client live fetches admit `NewsProvider::global_news`;
  any DNS, TLS, HTTP, MIME, parser, or provenance failure leaves the public
  capability false and preserves only the explicitly named diagnostic method.
- Router integration requires only `ProviderId::Yonhap` fixture evidence.
  Adding `magic-yonhap-rs` to Router production dependencies would violate the
  current architecture and compliance boundary.
- `tools/release/package.sh` derives no probe count automatically, so the two
  new Yonhap binaries must be added explicitly and every prose count must be
  reconciled against the actual `build_probe` call count.

## Official References

- RSS guide: <https://cn.yna.co.kr/channel/rss>
- Chinese terms: <https://cn.yna.co.kr/aboutus/copyright>
- Reference article:
  <https://cn.yna.co.kr/view/ACK20260629002400881>
- English initiating article:
  <https://en.yna.co.kr/view/AEN20260725001153315>
