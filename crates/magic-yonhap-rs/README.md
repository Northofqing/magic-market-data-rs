# magic-yonhap-rs

Read-only, metadata-only adapter for the seven official Yonhap
simplified-Chinese RSS feeds.

## Current capability

`YonhapClient::content_capabilities()` reports:

- `instrument_news = false`
- `global_news = false`

The deterministic parser and transport contract are implemented, but live
admission is currently false. On 2026-07-26 the release-built Rust client
received `tls connection init failed: unexpected end of file` from both the
Rolling and Economy feeds, including outside the sandbox.
`NewsProvider::global_news` therefore returns typed `Unsupported`.
`probe_global_news` is the explicitly named diagnostic path that performs a
bounded request and preserves the real transport failure.

## Official channels

| `YonhapChannel` | Probe spelling | Exact endpoint | Topic |
| --- | --- | --- | --- |
| `Rolling` | `rolling` | `https://cn.yna.co.kr/RSS/news.xml` | 滚动 |
| `Politics` | `politics` | `https://cn.yna.co.kr/RSS/politics.xml` | 政治 |
| `Economy` | `economy` | `https://cn.yna.co.kr/RSS/economy.xml` | 经济 |
| `Society` | `society` | `https://cn.yna.co.kr/RSS/society.xml` | 社会 |
| `CultureSports` | `culture-sports` | `https://cn.yna.co.kr/RSS/culture-sports.xml` | 文化体育 |
| `NorthKorea` | `north-korea` | `https://cn.yna.co.kr/RSS/nk.xml` | 朝鲜 |
| `ChinaKorea` | `china-korea` | `https://cn.yna.co.kr/RSS/china-relationship.xml` | 中韩关系 |

The client permits only these exact HTTPS URLs, port 443, and their exact
final response URLs. Credentials, redirects, alternate hosts, query strings,
fragments, and unregistered paths are rejected.

## Mapping and bounds

Each eligible RSS item maps:

- `/view/ACK` plus exactly 17 digits to `item_id`;
- decoded and whitespace-normalized RSS `title`;
- exact official HTTPS article URL;
- RFC 2822 publication time normalized to explicit `+09:00`;
- publisher `韩联社`, selected channel topic, language `zh-CN`;
- `ProviderId::Yonhap`, source time, observation time, and batch ID.

`summary` and `content` are always `None`; `instruments` is always empty.
Descriptions, `content:encoded`, media, and other extensions are discarded.
Article pages are never requested.

The caller limit is 1–50, the complete source feed is limited to 100 items,
the body limit is 2 MiB, timeout is 1–60 seconds, and cloned clients share a
one-second request-start gate held through response completion. The complete
feed must be valid, unique, and newest-first before caller-limit truncation.
DTD declarations and custom named entities are rejected.

## Diagnostics

```bash
cargo test -p magic-yonhap-rs --all-targets --locked --offline
cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline
MAGIC_YONHAP_CHANNEL=economy MAGIC_YONHAP_LIMIT=20 \
  cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline
MAGIC_YONHAP_CHANNEL=economy MAGIC_YONHAP_MATCH='半导体' \
  cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline
MAGIC_YONHAP_LOAD_REQUESTS=2 \
  cargo run -p magic-yonhap-rs --example load_probe --release --locked --offline
```

`MAGIC_YONHAP_MATCH` is a local, case-sensitive match over the current bounded
feed. It is not historical search and fails if the current window does not
contain the text. Load requests are serial and hard-capped at three.

Failures remain typed as invalid request, HTTPS transport, RSS decode, RSS
protocol, unsupported capability, or Core contract errors. No fixture, stale
cache, alternate Provider, or synthetic record replaces a real failure.

## Rights boundary

The official RSS directory is <https://cn.yna.co.kr/channel/rss>. Yonhap's
Chinese terms are <https://cn.yna.co.kr/aboutus/copyright>. This crate is an RSS
metadata reader, not a grant of display, storage, indexing, or redistribution
rights. It does not fetch, persist, cache, index, translate, or redistribute
article bodies. Deployers remain responsible for their own licensing and use
of titles and links.
