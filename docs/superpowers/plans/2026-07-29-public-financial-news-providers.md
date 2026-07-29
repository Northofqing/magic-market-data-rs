# Public Financial News Metadata Provider Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use executing-plans to implement this plan
> task-by-task. Follow red-green-refactor and keep normalized output
> metadata-only.

**Goal:** Add bounded first-party global-news Providers for Xinhua Finance,
Yicai, and Securities Times using the exact public listing contracts proven by
the source audit.

**Architecture:** Each crate owns one first-page source contract and implements
only `NewsProvider::global_news`. Xinhua and Yicai parse server-rendered first
pages; Securities Times calls the page-declared first-party quick-news JSON
route with standard JSON/XHR headers and omitted initial cursor keys. Every
parser validates the complete bounded source page before applying the caller
limit. Article bodies and descriptions are skipped and never retained.

**Tech Stack:** Rust 2021, Core `NewsItem`/`NewsProvider`, shared transport,
`serde_json`, `time 0.3.54`, strict marker scanning, official first-party HTTPS
pages.

---

## Task 1: Scaffold the three metadata-only news crates

**Files:**

- Modify: `Cargo.toml`
- Create: `crates/magic-xinhua-rs/Cargo.toml`
- Create: `crates/magic-xinhua-rs/src/lib.rs`
- Create: `crates/magic-xinhua-rs/tests/capabilities.rs`
- Create: `crates/magic-yicai-rs/Cargo.toml`
- Create: `crates/magic-yicai-rs/src/lib.rs`
- Create: `crates/magic-yicai-rs/tests/capabilities.rs`
- Create: `crates/magic-stcn-rs/Cargo.toml`
- Create: `crates/magic-stcn-rs/src/lib.rs`
- Create: `crates/magic-stcn-rs/tests/capabilities.rs`
- Modify: `Cargo.lock`

**Step 1: Register crates and manifests**

Use this common manifest shape:

```toml
[package]
name = "magic-xinhua-rs"
version = "0.2.0"
edition.workspace = true
license.workspace = true

[dependencies]
magic-market-core = { path = "../magic-market-core", version = "=0.2.0" }
magic-market-transport = { path = "../magic-market-transport", version = "=0.2.0" }
serde = { workspace = true }
serde_json = "1"
thiserror = { workspace = true }
time = { version = "=0.3.54", default-features = false, features = ["formatting", "parsing", "std"] }

[lints]
workspace = true
```

Change the package name for Yicai and STCN.

**Step 2: Write capability and unsupported-operation red tests**

For each crate:

```rust
#[test]
fn only_global_news_can_be_admitted() {
    let capabilities = Client::content_capabilities();
    assert!(!capabilities.instrument_news);
    assert_eq!(capabilities.global_news, GLOBAL_NEWS_ADMITTED);
    assert!(!capabilities.announcements);
    assert!(!capabilities.market_announcements);
    assert!(!capabilities.investor_questions);
}
```

Add an injected transport counter and prove `instrument_news` returns typed
`Unsupported` without I/O.

Run:

```bash
cargo test -p magic-xinhua-rs -p magic-yicai-rs -p magic-stcn-rs \
  --test capabilities --offline
```

Expected: unresolved clients/constants.

**Step 3: Implement common client shell**

Each client contains `Arc<dyn HttpTransport>` and `Arc<RequestGate>`, a
1-second interval, timeout `1..=60s`, and a source-specific maximum limit. Each
error enum contains `InvalidRequest`, transparent `Transport`, `Decode`,
`Protocol`, `Unsupported`, and transparent `Core`.

Start all `GLOBAL_NEWS_ADMITTED` flags false. Implement `global_news` so a false
flag returns `Unsupported`; add a separately named `probe_global_news` that
executes the real contract for admission.

**Step 4: Commit**

```bash
cargo update --offline
cargo check -p magic-xinhua-rs -p magic-yicai-rs -p magic-stcn-rs --offline
git add Cargo.toml Cargo.lock crates/magic-xinhua-rs crates/magic-yicai-rs \
  crates/magic-stcn-rs
git commit -m "feat: scaffold public financial news providers"
```

## Task 2: Implement Xinhua Finance server-rendered listing metadata

**Files:**

- Create: `crates/magic-xinhua-rs/src/html.rs`
- Create: `crates/magic-xinhua-rs/src/transport.rs`
- Create: `crates/magic-xinhua-rs/tests/fixtures/news.html`
- Create: `crates/magic-xinhua-rs/tests/html.rs`
- Create: `crates/magic-xinhua-rs/examples/live_probe.rs`
- Create: `crates/magic-xinhua-rs/examples/load_probe.rs`
- Modify: `crates/magic-xinhua-rs/src/lib.rs`

**Step 1: Write a source-shaped fixture**

The fixture includes 13 `.ui-zxlist-item` rows with one canonical link:

```html
<li class="ui-zxlist-item">
  <a href="/yw-lb/detail/20260729/4277771_1.html"
     title="合成的公开财经标题">合成的公开财经标题</a>
  <span class="ui-zxlist-time">2026-07-29 10:31:05</span>
  <span class="ui-zxlist-tag">要闻</span>
  <p>这段合成摘要必须被忽略。</p>
</li>
```

Use only synthetic titles and summaries, not copied publisher content.

**Step 2: Write red tests**

Prove:

- exactly one canonical `/yw-lb/detail/YYYYMMDD/{digits}_1.html` link per row;
- item ID is the numeric source ID;
- title/time/category are non-empty and exact;
- link date equals publication date;
- alternate host, query, fragment, unsafe path, duplicate ID, duplicate URL,
  missing time, malformed time, or more than 13 rows fails;
- all 13 rows are validated before a requested limit of 1 is applied;
- `summary=None`, `content=None`, instruments empty, language `zh-CN`;
- publisher is exact `新华财经`;
- the fixture summary phrase does not appear in serialized normalized records
  or probe formatting.

Run:

```bash
cargo test -p magic-xinhua-rs --test html --offline
```

Expected: unresolved parser.

**Step 3: Implement strict first-page parsing**

Limits:

```rust
const LIST_URL: &str = "https://www.cnfin.com/news/index.html";
const MAX_HTML_BYTES: usize = 1024 * 1024;
const MAX_SOURCE_ROWS: usize = 13;
const MAX_RETURNED_ITEMS: u32 = 13;
```

Scan exact list-item start/end markers, then exact anchor/time/category tags.
Decode only named entities `amp`, `lt`, `gt`, `quot`, `apos`, and numeric
Unicode entities with scalar validation. Reject nested/ambiguous markers and
unclosed tags. Never extract `<p>`.

Build evidence with the exact publication time as `source_at`; use the newest
row time for batch provenance `source_at`. Normalize the source's Beijing wall
time to RFC 3339 with the explicit `+08:00` offset, matching existing Chinese
news Providers; never interpret it as UTC. Preserve source order after proving
it is non-increasing by publication time.

**Step 4: Implement transport and probes**

Allow GET only to host `www.cnfin.com`, path `/news/index.html`, no query,
HTML MIME, 1 MiB, no redirect. `live_probe` prints provider, ID, title,
publisher, canonical URL, published/source/observed times, and batch ID.
`load_probe` performs exactly three serial first-page calls.

Run:

```bash
cargo test -p magic-xinhua-rs --all-targets --offline
cargo clippy -p magic-xinhua-rs --all-targets --offline -- -D warnings
git add crates/magic-xinhua-rs
git commit -m "feat(xinhua): implement first-page news metadata"
```

## Task 3: Implement Yicai embedded `firstlist` metadata

**Files:**

- Create: `crates/magic-yicai-rs/src/html.rs`
- Create: `crates/magic-yicai-rs/src/transport.rs`
- Create: `crates/magic-yicai-rs/tests/fixtures/news-info.html`
- Create: `crates/magic-yicai-rs/tests/html.rs`
- Create: `crates/magic-yicai-rs/examples/live_probe.rs`
- Create: `crates/magic-yicai-rs/examples/load_probe.rs`
- Modify: `crates/magic-yicai-rs/src/lib.rs`

**Step 1: Write a source-shaped synthetic fixture**

```html
<script>
var firstlist = [{
  "NewsID": 102765432,
  "NewsTitle": "合成的第一财经标题",
  "CreateDate": "2026-07-29 10:25:00",
  "NewsSource": "第一财经",
  "url": "/news/102765432.html",
  "NewsNotes": "这段合成正文必须被忽略。",
  "Image": "https://example.invalid/not-retained.jpg"
}];
</script>
```

**Step 2: Write extraction and metadata red tests**

Prove:

- exactly one `var firstlist =` assignment exists;
- balanced JSON array scanning honors quoted brackets and escapes;
- the JSON substring is capped before `serde_json::from_slice`;
- at most 300 source objects and 50 returned items;
- IDs, titles, exact dates, non-empty source publisher, and relative canonical
  URLs are required;
- canonical URL is exactly `https://www.yicai.com/news/{id}.html`;
- URL ID must equal `NewsID`;
- duplicates, alternate path, source mismatch, malformed time, or non-monotonic
  order fail;
- omitted source fields may be ignored, but `NewsNotes`, image, speech, video,
  creator, popularity, and share metadata never enter `NewsItem`;
- normalized summary/content remain `None`.

Run:

```bash
cargo test -p magic-yicai-rs --test html --offline
```

Expected: unresolved parser.

**Step 3: Implement bounded extraction and transport**

Constants:

```rust
const LIST_URL: &str = "https://www.yicai.com/news/info/";
const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_EMBEDDED_JSON_BYTES: usize = 512 * 1024;
const MAX_SOURCE_ROWS: usize = 300;
const MAX_RETURNED_ITEMS: u32 = 50;
```

Deserialize only `NewsID`, `NewsTitle`, `CreateDate`, `NewsSource`, and `url`;
Serde skips every other property without storing it. Retain the exact
non-empty `NewsSource` as `publisher` so syndicated material is not relabeled
as 第一财经. Normalize Beijing wall time to RFC 3339 `+08:00`. Allow GET only
to `www.yicai.com/news/info/`, no query, HTML MIME, no redirect.

**Step 4: Add probes, pass, and commit**

The probes follow the Xinhua output contract and serial count.

```bash
cargo test -p magic-yicai-rs --all-targets --offline
cargo clippy -p magic-yicai-rs --all-targets --offline -- -D warnings
git add crates/magic-yicai-rs
git commit -m "feat(yicai): implement embedded news metadata"
```

## Task 4: Implement Securities Times quick-news JSON metadata

**Files:**

- Create: `crates/magic-stcn-rs/src/json.rs`
- Create: `crates/magic-stcn-rs/src/transport.rs`
- Create: `crates/magic-stcn-rs/tests/fixtures/quick-news.json`
- Create: `crates/magic-stcn-rs/tests/json.rs`
- Create: `crates/magic-stcn-rs/examples/live_probe.rs`
- Create: `crates/magic-stcn-rs/examples/load_probe.rs`
- Modify: `crates/magic-stcn-rs/src/lib.rs`

**Step 1: Write the source-shaped fixture**

```json
{
  "state": 1,
  "data": [{
    "id": "4754321",
    "url": "/article/detail/4754321.html",
    "web_url": "https://www.stcn.com/article/detail/4754321.html",
    "title": "合成的人民财讯快讯标题",
    "source": "人民财讯",
    "time": 1785291905000,
    "show_time": 1785291905,
    "pageTime": 2,
    "content": "这段合成正文必须被忽略。",
    "share": {"description": "这段分享摘要也必须被忽略。"}
  }],
  "page_time": 2,
  "last_time": 1785291905000
}
```

**Step 2: Write red tests for envelopes and timestamps**

Prove:

- initial request has no `page_time` or `last_time` query key;
- `state == 1`, data is an array for a non-empty first page, and exactly 1
  through 30 rows;
- only a separately tested terminal shape may use `data:""` with null cursor;
- row ID equals both canonical URL IDs;
- `source == "人民财讯"`;
- `time / 1000 == show_time`;
- every `pageTime` equals envelope `page_time`;
- source rows and `last_time` are non-increasing/consistent;
- malformed cursors, duplicate IDs, over 30 rows, wrong publisher, alternate
  URL, or timestamp mismatch fails;
- full `content` and `share.description` are skipped and never appear in
  normalized serialization/probe formatting.

Run:

```bash
cargo test -p magic-stcn-rs --test json --offline
```

Expected: unresolved parser.

**Step 3: Implement the exact first request**

```text
GET https://www.stcn.com/article/list.html?type=kx
Accept: application/json, text/javascript, */*; q=0.01
X-Requested-With: XMLHttpRequest
Referer: https://www.stcn.com/article/list/kx.html
```

Allow only query key `type` and require value `kx`. Initial requests omit
cursor keys rather than serializing empty values. Use JSON MIME, 2 MiB ceiling,
no redirect, one-second shared pacing. Do not implement historical cursor
traversal in this slice.

Deserialize only:

```rust
struct Row {
    id: String,
    url: String,
    web_url: String,
    title: String,
    source: String,
    time: i64,
    show_time: i64,
    #[serde(rename = "pageTime")]
    page_time: u32,
}
```

Unknown content/share fields are parsed and discarded by Serde, never retained.
Convert the paired Unix timestamp to RFC 3339 with explicit `+08:00` for
`published_at` and evidence `source_at`.

**Step 4: Add probes, pass, and commit**

```bash
cargo test -p magic-stcn-rs --all-targets --offline
cargo clippy -p magic-stcn-rs --all-targets --offline -- -D warnings
git add crates/magic-stcn-rs
git commit -m "feat(stcn): implement quick-news metadata"
```

## Task 5: Run independent admission and document rights

**Files:**

- Modify: `crates/magic-xinhua-rs/src/lib.rs`
- Modify: `crates/magic-yicai-rs/src/lib.rs`
- Modify: `crates/magic-stcn-rs/src/lib.rs`
- Create: `crates/magic-xinhua-rs/README.md`
- Create: `crates/magic-yicai-rs/README.md`
- Create: `crates/magic-stcn-rs/README.md`

**Step 1: Run two live probes and one load probe per source**

```bash
cargo run -p magic-xinhua-rs --example live_probe --offline
cargo run -p magic-xinhua-rs --example live_probe --offline
cargo run -p magic-xinhua-rs --example load_probe --offline
cargo run -p magic-yicai-rs --example live_probe --offline
cargo run -p magic-yicai-rs --example live_probe --offline
cargo run -p magic-yicai-rs --example load_probe --offline
cargo run -p magic-stcn-rs --example live_probe --offline
cargo run -p magic-stcn-rs --example live_probe --offline
cargo run -p magic-stcn-rs --example load_probe --offline
```

Run with network access. Set each crate's `GLOBAL_NEWS_ADMITTED=true` only if
that source's two non-empty strict live batches and three-call serial load
probe pass. One source's result does not affect the other flags.

**Step 2: Document exact boundaries**

Each README records host/path, maximum bytes/rows/returned items, pacing,
publication/source time, live evidence date, and unsupported operations.
State explicitly:

- title/link/time metadata only;
- no article-page crawling;
- no summary/body/description/image/video retention;
- no cookie, login, CAPTCHA, subscriber, or push-notification endpoint;
- no inferred instrument identities;
- technical public access does not grant content redistribution rights.

**Step 3: Checkpoint and commit**

```bash
cargo fmt --all -- --check
cargo test -p magic-xinhua-rs -p magic-yicai-rs -p magic-stcn-rs \
  --all-targets --offline
cargo clippy -p magic-xinhua-rs -p magic-yicai-rs -p magic-stcn-rs \
  --all-targets --offline -- -D warnings
git diff --check
git add crates/magic-xinhua-rs crates/magic-yicai-rs crates/magic-stcn-rs
git commit -m "docs: record public financial news admission"
```
