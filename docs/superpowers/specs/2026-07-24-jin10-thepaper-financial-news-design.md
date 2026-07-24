# Jin10 and The Paper Financial News Provider Design

## Goal

Add two independent, read-only public-news providers:

- `magic-jin10-rs` for the public Jin10 7x24 financial flash stream.
- `magic-thepaper-rs` for native articles on The Paper's finance channel.

Both providers implement `NewsProvider::global_news`, preserve source identity and
timestamps, reject malformed or duplicate data, and ship deterministic tests plus bounded
live/load probes. Instrument/date-filtered news remains explicitly unsupported because
neither verified source contract supplies a structured instrument/date query.

## Verified upstream contracts

### Jin10

The live official web bundle calls:

`https://flash-api.jin10.com/get_flash_list?channel=-8200&vip=1`

with the official public headers `x-app-id: bVBF4FyRTn5NJF5n` and
`x-version: 1.0.0`. A bounded live request returned an `OK` envelope with 20 rows.
Rows expose a stable ID, absolute China-local source time, type, channels, importance,
tags, attribution, content, and optional article/source links. Locked VIP placeholders
are explicitly marked and do not contain public content.

Normalized canonical URLs use a verified first-party detail form:

`https://flash.jin10.com/detail/{id}`

The provider does not authenticate, request protected details, decrypt content, or claim
access to VIP data.

### The Paper

The verified finance channel is:

`https://www.thepaper.cn/channel_25951`

The server-rendered page embeds JSON in the `__NEXT_DATA__` script. The payload exposes
API status, finance-channel rows, stable `contId`, absolute millisecond publication time,
subchannel metadata, tags, and forward flags. Native article canonical URLs use:

`https://www.thepaper.cn/newsDetail_forward_{contId}`

The provider admits only native finance-channel articles. External forwards are outside
this provider's declared contract because the channel payload does not provide a reliable
original publisher field for every forwarded row.

## Alternatives considered

1. Parse both rendered HTML pages. This minimizes direct API coupling but makes field
   boundaries, entity decoding, and DOM changes harder to verify.
2. Use undocumented JSON endpoints for both sources. Jin10 has a directly verified
   first-party JSON call, but The Paper's internal pagination API is less stable than its
   server-rendered contract.
3. Use Jin10's verified JSON API and The Paper's embedded SSR JSON. This keeps structured
   parsing for both while limiting each provider to a first-party path observed on its
   live official page.

The design chooses option 3. There is no automatic HTML/API fallback: an upstream
contract change must remain an explicit failure rather than silently switching semantics.

## Architecture

Each source owns a separate workspace crate with no dependency from downstream
applications:

- `magic-jin10-rs`
  - bounded HTTPS transport and official-host allowlist;
  - Jin10 request builder and strict JSON parser;
  - `NewsProvider` implementation;
  - deterministic fixture tests;
  - bounded live and load probes.
- `magic-thepaper-rs`
  - bounded HTTPS transport and official-host allowlist;
  - exact `__NEXT_DATA__` extraction and strict JSON parser;
  - native finance-row filtering and `NewsProvider` implementation;
  - deterministic fixture tests;
  - bounded live and load probes.

`magic-market-core` gains `ProviderId::Jin10` and `ProviderId::ThePaper`. The generic
router remains provider-neutral; existing news adapters validate record identity,
batch identity, limits, and duplicates without adding production dependencies on either
crate.

Compliance, documentation, deployment packaging, and workspace registries gain both
providers and their probes.

## Data flow and normalization

### Common flow

1. Validate `limit` against the provider's positive maximum before I/O.
2. Serialize request starts through a client-shared limiter with a minimum one-second
   interval.
3. Permit only the exact official HTTPS origin, follow zero redirects, require HTTP 200,
   validate content type, and cap the body at 2 MiB.
4. Decode and validate the complete upstream envelope.
5. Normalize public records and reject duplicate source IDs.
6. Apply the requested output limit after eligibility filtering.
7. Construct `SourceEvidence` for every record and strict `DataBatch` provenance with
   separate source time, observation time, and batch ID.

### Jin10 mapping

- `item_id`: source `id`.
- `title`: non-empty source title; otherwise normalized public content.
- `summary`: absent.
- `content`: normalized public content.
- `publisher`: source attribution when supplied, otherwise `金十数据`.
- `canonical_url`: linked public article URL for type 2 when it is HTTPS; otherwise the
  first-party Jin10 flash-detail URL.
- `published_at`: validated source time converted to RFC 3339 with `+08:00`.
- `instruments`: empty because the public row does not provide a verified structured
  security identity.
- `topics`: deterministic labels derived from public tags plus the `重要` label when the
  source marks the row important.
- `language`: `zh-CN`.
- `evidence.provider`: `ProviderId::Jin10`.

Only public type 0 flashes and type 2 linked articles are eligible. Locked VIP rows are
excluded before the caller limit. A row claiming to be public but lacking usable public
content is a protocol error. The parser never incorporates protected titles/descriptions.

### The Paper mapping

- `item_id`: `contId`.
- `title`: `name`.
- `summary` and `content`: absent because the channel contract does not supply article
  text.
- `publisher`: `澎湃新闻`.
- `canonical_url`: first-party native detail URL derived from `contId`.
- `published_at`: `pubTimeLong` converted from epoch milliseconds to RFC 3339 `+08:00`.
- `instruments`: empty because tags and titles are not structured security identities.
- `topics`: subchannel name followed by unique source tags.
- `language`: `zh-CN`.
- `evidence.provider`: `ProviderId::ThePaper`.

Rows must have native-forward flags, no external link, positive publication time, and a
valid ID/title. Eligible rows are sorted by source time descending before applying the
caller limit because the page uses editorial recommendation order.

## Error behavior

Each crate exposes a typed provider error with:

- `InvalidRequest` for zero timeout, oversized limits, or invalid caller input.
- `Transport` for network, HTTP status, limiter-lock, and response-read failures.
- `Decode` for invalid JSON or invalid embedded JSON extraction.
- `Protocol` for envelope errors, content-type mismatches, duplicates, missing required
  fields, inconsistent public/VIP or native/forward flags, invalid times/URLs, oversized
  bodies, and empty eligible batches.
- `Unsupported` for instrument/date-filtered news.
- `Core` for normalization contract failures.

No provider error is converted into an empty success. HTTP 429 remains an explicit
transport/rate-limit failure and is never retried without pacing.

## Bounds and pacing

- Jin10 accepts `1..=20` requested public records because the verified endpoint returned
  a 20-row page.
- The Paper accepts `1..=20`; the current SSR page may return fewer eligible native rows,
  and the result is a bounded available subset rather than fabricated pagination.
- Both response bodies are capped at 2 MiB.
- Production clones share a request gate with request starts at least one second apart.
- Load probes default to two requests, cap at three, and use at most two threads.

## Testing and verification

Deterministic tests cover:

- capability declarations and explicit unsupported methods;
- official URL/header construction;
- successful mapping of every normalized field and evidence value;
- source-time conversion and newest-first output;
- duplicate ID rejection;
- malformed envelopes, content types, timestamps, URLs, and required fields;
- Jin10 locked VIP omission and refusal to use protected metadata;
- Jin10 type 0/type 2 handling and public-content requirements;
- The Paper exact `__NEXT_DATA__` extraction, channel ID/status validation, native-row
  admission, external-forward filtering, and tag deduplication;
- request bounds, response-size caps, shared pacing, and transport failures;
- provider identity serialization and generic router acceptance.

Verification runs formatting, all workspace tests, Clippy with warnings denied,
compliance, documentation-link checks, and bounded live probes for both providers.
Release claims are limited to capabilities proven by those checks.
