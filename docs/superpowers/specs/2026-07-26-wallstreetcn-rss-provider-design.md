# WallstreetCN RSS News Provider Design

## Objective

Add 华尔街见闻 (WallstreetCN) as a first-class, read-only news Provider using
the public RSS feed on its first-party domain. The Provider exposes only the
metadata required by the existing `NewsProvider::global_news` contract and
never returns, stores, indexes, or republishes article descriptions, bodies,
or images.

The feature serves bounded latest-article discovery. It is not a historical
archive, a full-text reader, a search engine, or an adapter for WallstreetCN's
undocumented web or application APIs.

## Source and Rights Boundary

The approved source is exactly:

`https://dedicated.wallstreetcn.com/rss.xml`

On 2026-07-26 the endpoint returned an RSS 2.0 document with 54 items. Each
item provided a title, canonical `https://wallstreetcn.com/articles/<id>`
link, `华尔街见闻` source label, RFC 2822 publication time, and a description
containing article content. The response used the inaccurate media type
`text/html; charset=UTF-8`.

WallstreetCN's [website](https://wallstreetcn.com/) states that its content
may not be copied, reproduced, or otherwise used without permission. Its
[user agreement](https://wallstreetcn.com/articles/3522782) also restricts
unauthorized third-party access and use of service data. The existence of a
public first-party RSS endpoint is not treated as a license to redistribute
article content.

The Provider therefore:

- reads only the one exact public RSS endpoint;
- maps title, numeric article identity, canonical URL, publication time, and
  publisher;
- skips every RSS `description` event without accumulating or exposing it;
- does not fetch article pages, images, media, or content extensions;
- does not use login state, cookies, browser sessions, hidden APIs, or
  application credentials;
- does not implement persistence, caching, historical search, full-text
  indexing, or content redistribution;
- documents that operators and downstream users remain responsible for any
  permission required for their use.

## Considered Approaches

### Standalone Provider crate — selected

Add `ProviderId::WallstreetCn` and a new `magic-wallstreetcn-rs` workspace
crate. This preserves exact provenance, isolates source-specific protocol
rules, and matches the existing Provider architecture.

### Shared RSS infrastructure refactor

Extract common RSS transport and parsing infrastructure and migrate Yonhap at
the same time. This could remove some duplication, but it expands the change
into a cross-Provider refactor before two source contracts have proved a
stable shared abstraction.

### Add WallstreetCN to an existing news Provider

This has the smallest file count but would misstate source identity and make
audit evidence ambiguous. It is rejected.

## Architecture

### Provider identity

`magic-market-core` adds `ProviderId::WallstreetCn`, with stable
`"WallstreetCn"` serialization. Every normalized record uses that identity in
`SourceEvidence`; batch provenance uses the stable source label
`wallstreetcn-rss-v1`.

### Provider crate

A new workspace member, `crates/magic-wallstreetcn-rs`, owns:

- `WallstreetCnClient`;
- exact request and endpoint validation;
- bounded HTTPS transport and clone-shared pacing;
- a strict streaming RSS parser;
- metadata-only `NewsItem` mapping;
- `NewsProvider` integration;
- an explicitly named diagnostic fetch path if live admission is not proved.

The crate depends only on `magic-market-core`, `thiserror`, the existing
`ureq` TLS stack, and the already locked `quick-xml` and `time` versions. It
does not depend on another Provider crate or any downstream project.

### Existing Router

The Provider uses the existing `global_news_source` adapter and
`GlobalNewsRouter`. Router receives no production dependency on
`magic-wallstreetcn-rs`. Provider-neutral Router fixtures prove that
WallstreetCN evidence is accepted only when record and batch identities
agree.

## Public API and Capability Semantics

The intended API is:

```rust
let client = WallstreetCnClient::new()?;
let batch = client.global_news(PositiveU32::new(20)?)?;
```

`instrument_news` returns typed `Unsupported`: the RSS feed provides no
verified exchange/instrument identity, and titles are not used to infer one.

`global_news` accepts a caller limit from 1 through 50. It validates the
complete bounded source feed before returning at most the requested number of
newest entries. A valid feed may contain fewer records; an empty feed is an
explicit protocol failure.

Capability admission follows repository evidence rules:

- deterministic fixtures prove parsing and failure behavior;
- the production Rust client must complete two consecutive bounded live
  fetches against the exact RSS endpoint;
- the requests remain subject to the shared one-second gate;
- every returned record must satisfy metadata-only and provenance checks;
- any DNS, TLS, timeout, HTTP, redirect, MIME, parser, order, or provenance
  failure keeps `content_capabilities().global_news` false;
- while false, `NewsProvider::global_news` returns typed `Unsupported`, and
  only the explicitly named diagnostic method performs network access.

No fixture result can promote a live capability.

## Request and Data Flow

1. Revalidate the caller limit before transport.
2. Acquire the clone-shared request gate.
3. Request the one exact HTTPS RSS URL with minimal stable headers.
4. Hold the gate through completion of the bounded response read.
5. Validate status, final URL, media type, size, XML declaration, RSS/channel
   structure, and channel identity.
6. Parse and validate every source item, skipping descriptions without
   retaining their text.
7. Reject malformed fields, invalid canonical URLs, duplicates, excess source
   rows, or source-order regressions.
8. Apply the caller limit only after the complete feed is valid.
9. Build strict record and batch evidence using the fetch-completion
   observation time.

There is no webpage, alternate host, undocumented API, fixture, or cache
fallback.

## Data Mapping

Each valid RSS item maps as follows:

| `NewsItem` field | Source and rule |
| --- | --- |
| `item_id` | Decimal article ID extracted from the canonical `/articles/<id>` URL |
| `title` | Non-empty RSS title after XML decoding and whitespace normalization |
| `summary` | Always `None` |
| `content` | Always `None` |
| `publisher` | Exact `华尔街见闻` |
| `canonical_url` | Exact `https://wallstreetcn.com/articles/<id>` with no credentials, port, query, fragment, alternate host, or extra path |
| `published_at` | Valid RFC 2822 source time normalized to ISO 8601 with an explicit offset |
| `instruments` | Empty |
| `topics` | One stable `华尔街见闻` label |
| `language` | `zh-CN` |
| `evidence` | `ProviderId::WallstreetCn`, source time, fetch-completion time, and stable batch ID |

The channel must have the exact title `华尔街见闻`, link
`https://wallstreetcn.com`, and language `zh-hans`. Every item must have the
exact source label `华尔街见闻`.

Rows are unique by both article ID and canonical URL and remain newest first.
Present-but-invalid required fields reject the whole batch. The parser never
turns malformed rows into a successful empty result.

## Transport and Resource Bounds

Production transport enforces:

- HTTPS only, host exactly `dedicated.wallstreetcn.com`, port 443, and path
  exactly `/rss.xml`;
- no credentials, query, fragment, redirect, cross-host final URL, or HTTP
  fallback;
- a closed response media-type set:
  `application/rss+xml`, `application/xml`, `text/xml`, and the source's
  currently observed `text/html`;
- when `text/html` is returned, the exact endpoint and the complete RSS
  structure are still mandatory; an HTML page cannot pass parsing;
- a 2 MiB response ceiling;
- a timeout configurable only from 1 through 60 seconds;
- at most 100 source items and 50 returned items;
- one shared request gate across clones, held through the complete response
  read, with request starts at least one second apart.

Invalid requests, transport failures, HTTP/redirect/MIME violations,
malformed data, unsupported capability, and Core contract failures remain
distinct typed errors.

## Probes

`examples/live_probe.rs` prints capability state, provenance, article IDs,
titles, canonical URLs, publication times, and confirmation that summary and
content are absent.

`MAGIC_WALLSTREETCN_LIMIT` selects a caller limit from 1 through 50.
`MAGIC_WALLSTREETCN_MATCH` performs an optional local, case-sensitive title
match after the bounded RSS fetch. It does not call a search endpoint and
fails explicitly if the current feed lacks the requested text.

`examples/load_probe.rs` is serial, defaults to two requests, permits at most
three, and verifies that the client-enforced one-second pacing boundary
remains active.

Probe output never prints descriptions, article bodies, cookies, tokens, or
personal information.

## Testing

All checked-in RSS fixtures are synthetic and contain no copied WallstreetCN
article text. Deterministic tests cover:

- exact endpoint, request headers, timeout, limit, final URL, media types, and
  response/body bounds;
- valid metadata-only mapping and exact channel/source identity;
- descriptions, media, and extension content being discarded;
- empty and oversized feeds, malformed XML, DTDs, custom entities, control
  references, invalid encoding, and wrong RSS/channel structure;
- missing fields, noncanonical article URLs, ID overflow, duplicate IDs/URLs,
  bad times, and source-order regressions;
- complete-feed validation before local truncation;
- clone-shared pacing held through response completion;
- typed `instrument_news` and pre-admission `global_news` behavior;
- truthful capability reporting and Router identity enforcement.

Live evidence is recorded separately. A live failure remains a typed failure
and is never replaced by fixture data.

## Documentation and Release Gates

The implementation updates:

- workspace membership and the Core Provider identity contract;
- the root capability matrix and probe commands;
- `crates/magic-wallstreetcn-rs/README.md`;
- `docs/integrations/wallstreetcn-rss.md`;
- deployment host/path and health requirements;
- business rules and upstream provenance;
- compliance and package scripts;
- strict coverage through normal workspace source discovery.

Before release, run rustfmt, locked offline all-target check and test, strict
Clippy, Rustdoc and doctests, documentation links, compliance, dependency
boundary checks, strict production coverage, and the complete release
preflight.

## Non-Goals

- RSS descriptions, article bodies, excerpts, images, audio, or video;
- article-page crawling or undocumented API access;
- WallstreetCN fast-news/快讯 ingestion;
- authenticated, VIP, or paid content;
- historical search, storage, caching, indexing, or redistribution;
- automatic instrument or topic inference from prose;
- bypassing source access controls, licensing, robots, rate limits, or
  capability admission.
