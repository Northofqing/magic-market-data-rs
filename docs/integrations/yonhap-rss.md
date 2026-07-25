# Yonhap simplified-Chinese RSS integration

## Scope and admission state

`magic-yonhap-rs` is a first-class, read-only Provider for official Yonhap
simplified-Chinese RSS metadata. It does not crawl article pages and does not
implement historical search, persistence, caching, translation, or security
identity inference.

As of 2026-07-26:

- deterministic request, transport, parser, pacing, metadata, capability, and
  Router evidence tests pass;
- `instrument_news=false`;
- `global_news=false`;
- `NewsProvider::global_news` returns typed `Unsupported`;
- `YonhapClient::probe_global_news` is the only explicit network diagnostic.

The release-built production Rust client received
`tls connection init failed: unexpected end of file` from both Rolling and
Economy RSS, including outside the sandbox. BR-021 therefore prohibits a live
capability claim. Fixture success does not replace production endpoint
evidence.

## Official endpoint allowlist

| Channel | Environment value | Exact URL |
| --- | --- | --- |
| Rolling | `rolling` | `https://cn.yna.co.kr/RSS/news.xml` |
| Politics | `politics` | `https://cn.yna.co.kr/RSS/politics.xml` |
| Economy | `economy` | `https://cn.yna.co.kr/RSS/economy.xml` |
| Society | `society` | `https://cn.yna.co.kr/RSS/society.xml` |
| Culture and sports | `culture-sports` | `https://cn.yna.co.kr/RSS/culture-sports.xml` |
| North Korea | `north-korea` | `https://cn.yna.co.kr/RSS/nk.xml` |
| China–Korea relations | `china-korea` | `https://cn.yna.co.kr/RSS/china-relationship.xml` |

Requests and final response URLs must match this table exactly. The production
transport enforces HTTPS, host `cn.yna.co.kr`, port 443 by exact URL, zero
redirects, and no credentials, alternate host, query, fragment, or
caller-composed path.

Responses require HTTP 200 and one of:

- `application/rss+xml`;
- `application/xml`;
- `text/xml`.

MIME parameters are allowed. Missing MIME, HTML, JSON, lookalike MIME values,
wrong final URLs, or a body over 2 MiB fail explicitly.

## Request and resource contract

- one selected feed per request;
- caller limit 1–50;
- complete source-feed limit 100 items;
- timeout 1–60 seconds, default 10;
- one shared request gate across clones;
- request starts at least one second apart;
- gate held through complete response acquisition;
- no hidden retry, alternate endpoint, stale cache, or cross-Provider fallback.

The load probe is serial, defaults to two requests, and permits at most three.

## Strict XML and row validation

The streaming parser:

1. requires UTF-8 and a complete `rss/channel/item` structure;
2. rejects malformed XML, DTD declarations, custom named entities, invalid
   numeric references, nested required fields, and duplicate required fields;
3. reads only direct item `title`, `link`, optional `guid`, and `pubDate`;
4. ignores `description`, `content:encoded`, media, and unknown extensions;
5. requires canonical `https://cn.yna.co.kr/view/ACK<17 ASCII digits>` links;
6. requires a present GUID to equal the article ID or canonical URL;
7. parses RFC 2822 time and normalizes it to RFC 3339 with `+09:00`;
8. requires unique article IDs and URLs in non-increasing source-time order;
9. validates all source items before truncating to the caller limit.

An empty feed, malformed trailing row, or 101st item is a protocol failure, not
a successful empty or partial result.

## Normalized fields

| `NewsItem` field | Mapping |
| --- | --- |
| `item_id` | `ACK` plus 17 digits from the canonical URL |
| `title` | decoded, non-empty, whitespace-normalized RSS title |
| `summary` | always `None` |
| `content` | always `None` |
| `publisher` | `韩联社` |
| `canonical_url` | exact official `/view/ACK...` HTTPS URL |
| `published_at` | validated explicit `+09:00` source time |
| `instruments` | empty |
| `topics` | one label from the selected channel |
| `language` | `zh-CN` |
| `evidence` | `ProviderId::Yonhap`, source/observed times, shared batch ID |

Batch source is `yonhap-cn-rss-v1`. Record and batch IDs agree, the latest
selected source time is preserved at batch level, and the batch is strict only
after every source row passes validation.

## Probe operation

```bash
cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline

MAGIC_YONHAP_CHANNEL=economy \
MAGIC_YONHAP_LIMIT=20 \
cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline

MAGIC_YONHAP_CHANNEL=economy \
MAGIC_YONHAP_MATCH='半导体' \
cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline

MAGIC_YONHAP_LOAD_REQUESTS=2 \
cargo run -p magic-yonhap-rs --example load_probe --release --locked --offline
```

The live probe prints capability state, batch provenance, item IDs, titles,
canonical URLs, source times, topics, Provider evidence, and explicit
summary/content absence.

`MAGIC_YONHAP_MATCH` performs a local case-sensitive title match only after a
bounded current-feed fetch. A miss means only that the current RSS window did
not contain the text; it is not evidence that the story never existed.

## Failure semantics

| Error | Meaning |
| --- | --- |
| `InvalidRequest` | invalid timeout/limit or attempted non-allowlisted request |
| `Transport` | TLS, connection, HTTP status, body-read, or request-gate failure |
| `Decode` | malformed/non-UTF-8 XML or invalid XML declaration/attributes |
| `Protocol` | wrong MIME/final URL, oversize body, bad structure/row/time/order/evidence |
| `Unsupported` | unadmitted global trait or unsupported instrument filter |
| `Core` | normalized value/evidence contract rejection |

Errors remain visible and typed. The adapter does not return fixture data,
scrape another language site, fetch the article, or relabel another Provider.

## Rights and provenance

Official source references reviewed on 2026-07-25:

- RSS directory: <https://cn.yna.co.kr/channel/rss>
- Chinese terms: <https://cn.yna.co.kr/aboutus/copyright>

The integration uses RSS metadata only and does not copy Yonhap source code.
It is not a license grant for article storage, display, indexing, or
redistribution. Deployers must confirm their own permitted use.
