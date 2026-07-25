# Yonhap Chinese RSS News Provider Design

## Objective

Add Yonhap News Agency as a first-class, read-only news Provider using the
official simplified-Chinese RSS feeds. The Provider exposes only source
metadata needed by the existing `NewsProvider::global_news` contract and never
fetches, stores, or republishes Yonhap article bodies.

The initiating acceptance case is a financial headline such as the
US$950-billion semiconductor cooperation story. The Provider must make current
RSS entries discoverable with stable provenance; it is not a historical
archive or search engine.

## Source and Rights Boundary

Yonhap publishes an official RSS guide at
<https://cn.yna.co.kr/channel/rss>. It lists these feeds:

| Channel | Exact feed |
| --- | --- |
| Rolling | `https://cn.yna.co.kr/RSS/news.xml` |
| Politics | `https://cn.yna.co.kr/RSS/politics.xml` |
| Economy | `https://cn.yna.co.kr/RSS/economy.xml` |
| Society | `https://cn.yna.co.kr/RSS/society.xml` |
| Culture and sports | `https://cn.yna.co.kr/RSS/culture-sports.xml` |
| North Korea | `https://cn.yna.co.kr/RSS/nk.xml` |
| China–Korea relations | `https://cn.yna.co.kr/RSS/china-relationship.xml` |

The same guide describes RSS as a mechanism for RSS readers to receive latest
messages. Yonhap's Chinese terms at
<https://cn.yna.co.kr/aboutus/copyright> prohibit unauthorized copying,
storage, transmission, display, or information-service redistribution of its
articles.

The adapter therefore:

- reads only one official RSS feed per request;
- maps title, official article identity and URL, publication time, and channel;
- ignores RSS `description`, CDATA body, media, and any content extension;
- does not fetch article pages;
- does not implement persistence, caching, search indexing, or redistribution;
- documents that downstream users remain responsible for obtaining any
  required license.

## Architecture

### Provider identity

`magic-market-core` adds `ProviderId::Yonhap`. Every normalized record uses
that identity in `SourceEvidence`; the batch provenance uses a stable
`yonhap-cn-rss-v1` source label.

### Provider crate

A new workspace member, `crates/magic-yonhap-rs`, owns:

- `YonhapChannel`, a closed enum for the seven documented feeds;
- `YonhapClient`, defaulting to `Rolling` and configurable with one channel;
- exact request construction and host/path validation;
- bounded HTTPS transport and shared pacing;
- a streaming RSS parser that rejects DTDs and external-entity constructs;
- `NewsProvider` integration;
- an explicit diagnostic fetch path if live admission remains unproved.

The crate depends only on `magic-market-core`, `thiserror`, the existing
`ureq` TLS stack, and one pinned streaming XML parser. It does not depend on
another Provider crate or add a downstream project path dependency.

### Existing Router

The Provider uses the existing `global_news_source` adapter and
`GlobalNewsRouter`. No Router production dependency on `magic-yonhap-rs` is
allowed. Router tests add Yonhap as a fixture identity and prove mismatched
record evidence is rejected.

## Public API and Capability Semantics

The intended API is:

```rust
let client = YonhapClient::new()?;
let economy = YonhapClient::for_channel(YonhapChannel::Economy)?;
let batch = economy.global_news(PositiveU32::new(20)?)?;
```

`instrument_news` returns typed `Unsupported`: RSS items do not provide a
verified market instrument identity.

`global_news` has a caller limit of at most 50. It fetches and validates the
complete bounded source feed before returning at most the requested number of
newest eligible entries. A feed with fewer entries may return fewer records;
an empty feed is an explicit protocol failure.

Capability admission follows the repository's evidence rule:

- deterministic fixtures prove parsing and failure behavior;
- the production Rust client must pass a bounded live probe against an
  official RSS endpoint before `content_capabilities().global_news` becomes
  `true`;
- if the live probe cannot be completed, capability remains `false`,
  `NewsProvider::global_news` returns typed `Unsupported`, and only the
  explicitly named diagnostic method performs network access.

## Data Mapping

Each eligible RSS item maps as follows:

| `NewsItem` field | Source and rule |
| --- | --- |
| `item_id` | Stable Yonhap article ID extracted from the canonical `/view/ACK...` URL; GUID must agree when present |
| `title` | Non-empty RSS title after XML decoding and whitespace normalization |
| `summary` | Always `None` |
| `content` | Always `None` |
| `publisher` | `韩联社` |
| `canonical_url` | Exact `https://cn.yna.co.kr/view/ACK...` URL with no credentials, fragment, alternate host, or unverified path |
| `published_at` | Validated RSS source time normalized to an explicit ISO-8601 offset |
| `instruments` | Empty; no textual company-name inference |
| `topics` | One stable Chinese label derived from `YonhapChannel` |
| `language` | `zh-CN` |
| `evidence` | `ProviderId::Yonhap`, source time, fetch-completion observation time, and stable batch ID |

Rows must be unique by both article ID and canonical URL and remain newest
first. Present-but-invalid required fields reject the batch. The parser never
turns malformed rows into a successful empty result.

## Transport and Resource Bounds

Production transport enforces:

- HTTPS only, host exactly `cn.yna.co.kr`, port 443, and one of the seven exact
  `/RSS/*.xml` paths;
- no credentials, query, fragment, redirect, cross-host final URL, or fallback
  to HTTP;
- XML-compatible response media type;
- a 2 MiB response ceiling;
- a timeout configurable within 1–60 seconds;
- a maximum of 100 source items and 50 returned items;
- one shared request gate across client clones, held through the complete
  response read, with request starts at least one second apart.

Transport, decode, protocol, unsupported, invalid-request, and Core failures
remain distinct typed errors.

## Probes

`examples/live_probe.rs` prints capability state, provenance, item IDs,
titles, URLs, timestamps, channel topics, and confirms that summary/content are
absent. Environment variables select the channel and caller limit.

`MAGIC_YONHAP_MATCH` performs a local case-sensitive title match after the
bounded RSS fetch. It is diagnostic convenience only, does not call a search
endpoint, and fails explicitly when the current feed does not contain the
requested text.

`examples/load_probe.rs` is serial, defaults to two requests, permits at most
three, and verifies the one-second client pacing boundary.

## Testing

Deterministic tests cover:

- every channel-to-endpoint mapping;
- a valid RSS fixture with entity decoding, stable article ID, explicit time,
  source order, metadata-only mapping, and provenance;
- descriptions and content extensions being ignored;
- empty feeds, oversized feeds/bodies, malformed XML, DTD/entity declarations,
  duplicate IDs/URLs, missing fields, wrong article hosts/paths, bad times, and
  source-order regressions;
- caller limit and timeout bounds before transport;
- exact content type, final URL, redirect, and clone-shared pacing behavior;
- `instrument_news` typed `Unsupported`;
- capability truthfulness and Router identity enforcement.

The live result is recorded separately from deterministic tests. A transport
failure is reported as evidence, never replaced by fixture data.

## Documentation and Release Gates

The change updates:

- the root capability matrix and probe commands;
- a dedicated `crates/magic-yonhap-rs/README.md`;
- `docs/integrations/yonhap-rss.md`;
- deployment host requirements;
- compliance workspace and documentation registration;
- coverage source discovery through normal workspace membership.

Before release, run formatting, all-target tests, workspace Clippy, Rustdoc and
doc tests, compliance, documentation links, strict coverage, and the complete
release preflight.

## Non-Goals

- article body or image retrieval;
- historical search or crawling article pages;
- storage, caching, or indexing;
- automatic stock-instrument inference;
- translating Korean or English stories;
- combining multiple RSS channels into one request;
- bypassing Yonhap licensing, robots, TLS, or access controls.
