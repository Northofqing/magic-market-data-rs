# WallstreetCN public RSS integration

## Scope and admission state

`magic-wallstreetcn-rs` is a first-class, read-only Provider for current
华尔街见闻 article metadata from exactly:

`https://dedicated.wallstreetcn.com/rss.xml`

It is not an article reader, fast-news client, historical search service,
archive, cache, or index. It does not use article pages, undocumented APIs,
cookies, browser/login state, authenticated content, or inferred security
identities.

On 2026-07-26 the release production client passed one bounded metadata probe
and two consecutive serial fetches through one shared gate. Consequently:

- `global_news=true`;
- `instrument_news=false`;
- `NewsProvider::global_news` returns strict metadata batches;
- `instrument_news` returns typed `Unsupported`;
- `probe_global_news` remains an explicit diagnostic entry point.

Fixture success alone did not admit the capability.

## Exact transport contract

Both the request URL and final response URL must equal the source URL byte for
byte. HTTPS, the first-party host, standard port, `/rss.xml`, no credentials,
no query/fragment, no alternate path, and zero redirects are enforced.

The source was observed returning the inaccurate media type
`text/html; charset=UTF-8`. The closed accepted base-MIME set is:

- `application/rss+xml`;
- `application/xml`;
- `text/xml`;
- `text/html`.

Parameters and ASCII case are normalized. Missing MIME, JSON, plain text,
wrong final URL, empty body, or HTTP/status failure is explicit. Accepting
`text/html` does not accept an HTML page: the exact endpoint and a complete,
strict RSS 2.0 document remain mandatory.

Resource limits:

- response body at most 2 MiB;
- caller limit 1–50;
- complete source feed at most 100 items;
- timeout 1–60 seconds, default 10;
- one clone-shared request gate;
- request starts at least one second apart;
- the gate remains held until the bounded response finishes.

There is no hidden retry, alternate endpoint, cached response, fixture, or
cross-Provider fallback.

## Strict RSS validation

The streaming parser requires UTF-8 and exactly one RSS 2.0 root/channel.
Channel identity must be:

| Field | Required value |
| --- | --- |
| `title` | `华尔街见闻` |
| `link` | `https://wallstreetcn.com` |
| `language` | `zh-hans` |

Each direct item requires `title`, `link`, `source`, and `pubDate`.
`source` must equal `华尔街见闻`. Article links must be exactly
`https://wallstreetcn.com/articles/<1-20 ASCII decimal digits>`, with no
credentials, port, query, fragment, alternate host, or extra path.

The parser rejects malformed/incomplete XML, DTDs, custom entities, invalid
numeric/control references, nested or duplicate required fields, duplicate
article identities/URLs, bad RFC 2822 times, order regression, empty feeds,
and a 101st source row. It validates every source row before applying the
caller limit, so a malformed trailing row cannot be hidden by truncation.

`description`, `content:encoded`, media, and extension subtrees are consumed
only for structure. Their text is never accumulated, normalized, mapped,
printed, stored, cached, or indexed.

## Normalized metadata

| `NewsItem` field | Mapping |
| --- | --- |
| `item_id` | decimal ID from the canonical article URL |
| `title` | non-empty decoded and whitespace-normalized RSS title |
| `summary` | always `None` |
| `content` | always `None` |
| `publisher` | exact `华尔街见闻` |
| `canonical_url` | exact first-party `/articles/<id>` HTTPS URL |
| `published_at` | RFC 2822 source time formatted as RFC 3339 with its offset |
| `instruments` | empty |
| `topics` | one `华尔街见闻` label |
| `language` | `zh-CN` |
| `evidence` | `ProviderId::WallstreetCn`, source/observed time, shared batch ID |

Batch provenance is `wallstreetcn-rss-v1`. Records and provenance share the
same batch ID, and the newest returned publication time is retained as batch
source time.

## Probes and admission evidence

```bash
cargo run -p magic-wallstreetcn-rs --example live_probe --release --locked --offline

MAGIC_WALLSTREETCN_LIMIT=20 MAGIC_WALLSTREETCN_MATCH='半导体' \
cargo run -p magic-wallstreetcn-rs --example live_probe --release --locked --offline

MAGIC_WALLSTREETCN_LOAD_REQUESTS=2 \
cargo run -p magic-wallstreetcn-rs --example load_probe --release --locked --offline
```

`MAGIC_WALLSTREETCN_LIMIT` defaults to 20 and accepts 1–50.
`MAGIC_WALLSTREETCN_MATCH` is an optional, non-empty, case-sensitive local
title match after the current bounded fetch; a miss is not evidence that a
story never existed. `MAGIC_WALLSTREETCN_LOAD_REQUESTS` defaults to 2 and
accepts 1–3 serial requests.

The live probe prints capabilities, batch provenance, IDs, quoted titles,
publisher, canonical URL, source time, topic, language, Provider evidence, and
booleans proving summary/content absence. It never prints descriptions or
article bodies.

Admission evidence on 2026-07-26:

- live: 20 strict metadata-only rows, complete batch;
- serial load: 2/2 requests, 10 rows each, 7.529 seconds total;
- all records used `ProviderId::WallstreetCn`, matching batch IDs, and absent
  summary/content;
- a local `9500亿美元` title match found article ID `3777926`.

One later optional title-match attempt had a typed DNS `Transport` failure; the
unchanged bounded probe passed on the permitted outside-sandbox retry.
Transient failures remain observable and do not trigger fallback.

## Failure semantics

| Error | Meaning |
| --- | --- |
| `InvalidRequest` | timeout/limit or exact-endpoint violation |
| `Transport` | DNS, TLS, connection, timeout, body-read, or request-gate failure |
| `Decode` | malformed/non-UTF-8 XML, declaration, name, or attribute |
| `Protocol` | status/final URL/MIME/body/RSS/field/time/order/provenance violation |
| `Unsupported` | instrument filtering or another undeclared capability |
| `Core` | normalized value/evidence contract rejection |

Errors stay typed and visible. The Provider does not substitute another
website, fixture, cache, or stale batch.

## Rights and operator responsibility

First-party references reviewed on 2026-07-26:

- public RSS feed: <https://dedicated.wallstreetcn.com/rss.xml>
- publisher website: <https://wallstreetcn.com/>
- user agreement: <https://wallstreetcn.com/articles/3522782>

The public feed is not treated as permission to copy or redistribute article
content. The adapter exposes metadata only and contains no WallstreetCN source
code, private API, login state, description, or article body. Operators and
downstream users must confirm the permissions, display rights, retention,
rate, and redistribution terms applicable to their own use.
