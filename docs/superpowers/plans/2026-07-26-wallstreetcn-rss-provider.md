# WallstreetCN RSS News Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a first-class, provenance-preserving WallstreetCN RSS Provider that exposes bounded latest-article metadata without returning, storing, indexing, or redistributing descriptions or article bodies.

**Architecture:** Add `ProviderId::WallstreetCn` and a standalone `magic-wallstreetcn-rs` crate split into client, transport, and RSS parser modules. Use one exact first-party RSS endpoint, strict complete-feed validation, clone-shared pacing, an injected transport seam, and evidence-gated `NewsProvider` admission while keeping Router production code provider-neutral.

**Tech Stack:** Stable Rust 2021, `magic-market-core`, `magic-market-router` fixture tests, `ureq 2.12.1` with Rustls, `quick-xml 0.41.0`, `time 0.3.54`, `thiserror 2`, Cargo workspace gates, `cargo-llvm-cov`.

---

## Approved Boundaries

- The only remote source is
  `https://dedicated.wallstreetcn.com/rss.xml`.
- Map only title, decimal article ID, exact canonical URL, publication time,
  publisher, language, topic, and provenance.
- Never map or print RSS `description`, article content, excerpts, images,
  audio, or video.
- Never fetch an article page, undocumented API, fast-news endpoint,
  authenticated endpoint, or paid content.
- Do not add persistence, caching, historical search, full-text indexing,
  inferred instruments, or text-derived market identities.
- Parse and validate the complete bounded source feed before applying the
  caller limit.
- Keep live admission evidence separate from deterministic fixture tests.
- Preserve typed failures; never replace a live error with fixture success.
- Do not add a Provider dependency to Router or a downstream
  `stock_analysis` path dependency.

## File and Responsibility Map

- Modify `crates/magic-market-core/src/provider.rs`: add the first-class
  `ProviderId::WallstreetCn` identity.
- Modify `crates/magic-market-core/tests/provider_identity.rs`: prove stable
  serialization and inclusion among intelligence sources.
- Modify `crates/magic-market-router/tests/intelligence_routing.rs`: prove
  provider-neutral acceptance and evidence-mismatch rejection.
- Modify `Cargo.toml`: register `crates/magic-wallstreetcn-rs`.
- Create `crates/magic-wallstreetcn-rs/Cargo.toml`: standalone Provider
  manifest with only Core and registry dependencies.
- Create `crates/magic-wallstreetcn-rs/src/lib.rs`: public error, client,
  capability, pacing, and `NewsProvider` contract.
- Create `crates/magic-wallstreetcn-rs/src/transport.rs`: exact request model,
  response bounds, production HTTPS transport, endpoint/final-URL/MIME
  validation, and injected transport trait.
- Create `crates/magic-wallstreetcn-rs/src/rss.rs`: strict RSS state machine,
  metadata-only mapping, complete-feed validation, and provenance creation.
- Create `crates/magic-wallstreetcn-rs/tests/capabilities.rs`: public contract,
  live-admission truth, and no-network unsupported tests.
- Create `crates/magic-wallstreetcn-rs/examples/live_probe.rs`: bounded
  metadata-only diagnostic and local title match.
- Create `crates/magic-wallstreetcn-rs/examples/load_probe.rs`: serial,
  at-most-three-request pacing diagnostic.
- Create `crates/magic-wallstreetcn-rs/README.md`: API, limits, capability
  state, rights boundary, and probe usage.
- Create `docs/integrations/wallstreetcn-rss.md`: exact source contract,
  mapping, failure behavior, probes, and admission evidence.
- Modify `README.md`: workspace tree, capability matrix, commands, packaged
  probes, network requirements, and acceptance status.
- Modify `docs/DEPLOYMENT.md`: package count, binaries, host/path allowlist,
  health checks, and no-cache boundary.
- Modify `docs/business_rules.md`: register BR-022.
- Modify `docs/UPSTREAM.md`: record the independent RSS implementation and
  first-party source/terms provenance.
- Modify `tools/compliance/check.sh`: require the crate, docs, BR-022, workspace
  membership, and Router neutrality.
- Modify `tools/release/package.sh`: package WallstreetCN live/load probes.
- Create
  `.planning/2026-07-26-wallstreetcn-rss-provider/{task_plan,findings,progress}.md`:
  persist execution status, source observations, failures, and gate evidence.

### Task 1: First-Class Provider Identity and Provider-Neutral Routing

**Files:**
- Modify: `crates/magic-market-core/src/provider.rs`
- Modify: `crates/magic-market-core/tests/provider_identity.rs`
- Modify: `crates/magic-market-router/tests/intelligence_routing.rs`
- Create:
  `.planning/2026-07-26-wallstreetcn-rss-provider/task_plan.md`
- Create:
  `.planning/2026-07-26-wallstreetcn-rss-provider/findings.md`
- Create:
  `.planning/2026-07-26-wallstreetcn-rss-provider/progress.md`

- [ ] **Step 1: Create persistent execution records**

Create a task plan with four phases: identity/transport, parser/contract,
admission/docs, and gates/review. Set Phase 1 in progress. Record these already
verified source facts in `findings.md`:

```markdown
- Exact feed: `https://dedicated.wallstreetcn.com/rss.xml`.
- 2026-07-26 bounded probe: HTTP 200, 359627 bytes, 54 items.
- Observed media type: `text/html; charset=UTF-8`.
- Required channel identity: title `华尔街见闻`, link
  `https://wallstreetcn.com`, language `zh-hans`.
- Required item fields: title, `/articles/{decimal_id}` link, source
  `华尔街见闻`, and RFC 2822 `pubDate`.
- RSS descriptions contain article content and are forbidden output.
```

Record the approved design commit `c2f6348` and this plan path in
`progress.md`.

- [ ] **Step 2: Write failing Core identity tests**

Add `ProviderId::WallstreetCn` to
`intelligence_sources_have_first_class_identities`, update the expected length
from 12 to 13, and add:

```rust
assert_eq!(
    serde_json::to_string(&ProviderId::WallstreetCn).unwrap(),
    "\"WallstreetCn\""
);
```

- [ ] **Step 3: Write failing Router identity tests**

Add a valid fixture test:

```rust
#[test]
fn global_news_router_accepts_wallstreetcn_identity() {
    let provider = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::WallstreetCn,
        batch_source: "wallstreetcn-rss-v1",
        item_count: 2,
        duplicate_id: false,
    });
    let mut router =
        GlobalNewsRouter::new(AcceptancePolicy::new().with_require_source_at(true));
    router
        .register(global_news_source(
            ProviderId::WallstreetCn,
            provider,
            classify,
        ))
        .unwrap();

    let outcome = router.route(&PositiveU32::new(2).unwrap()).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::WallstreetCn);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Selected
    ));
}
```

Add a mismatch test that registers `ProviderId::WallstreetCn` but returns a
record using `ProviderId::ThePaper`. Register a valid The Paper fixture after
it and assert the first attempt is rejected with `FailureKind::Evidence` and
The Paper is selected.

- [ ] **Step 4: Run the red tests**

Run:

```bash
cargo test -p magic-market-core --test provider_identity --locked --offline
cargo test -p magic-market-router --test intelligence_routing --locked --offline
```

Expected: compilation fails only because `ProviderId::WallstreetCn` does not
exist. Preserve both command results in `progress.md`.

- [ ] **Step 5: Add the Core identity**

Add the new identity immediately after the existing financial-news identities:

```rust
    Jin10,
    ThePaper,
    Yonhap,
    WallstreetCn,
    Sse,
```

Do not add a Serde rename; the stable wire value is exactly
`"WallstreetCn"`.

- [ ] **Step 6: Run focused green verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p magic-market-core --test provider_identity --locked --offline
cargo test -p magic-market-router --test intelligence_routing --locked --offline
git diff --check
```

Expected: every command exits zero; the Router test count increases by two.

- [ ] **Step 7: Commit the identity boundary**

```bash
git add crates/magic-market-core/src/provider.rs \
  crates/magic-market-core/tests/provider_identity.rs \
  crates/magic-market-router/tests/intelligence_routing.rs \
  .planning/2026-07-26-wallstreetcn-rss-provider
git commit -m "feat(core): add WallstreetCN provider identity"
```

### Task 2: Exact Request Model and Bounded HTTPS Transport

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/magic-wallstreetcn-rs/Cargo.toml`
- Create: `crates/magic-wallstreetcn-rs/src/lib.rs`
- Create: `crates/magic-wallstreetcn-rs/src/transport.rs`

- [ ] **Step 1: Register the crate and manifest**

Add `"crates/magic-wallstreetcn-rs"` after the Yonhap workspace member. Create:

```toml
[package]
name = "magic-wallstreetcn-rs"
version = "0.2.0"
edition.workspace = true
license.workspace = true

[dependencies]
magic-market-core = { path = "../magic-market-core", version = "=0.2.0" }
quick-xml = { version = "=0.41.0", default-features = false }
thiserror = { workspace = true }
time = { version = "=0.3.54", default-features = false, features = ["formatting", "parsing", "std"] }
ureq = { version = "=2.12.1", default-features = false, features = ["tls"] }

[lints]
workspace = true
```

Run `cargo metadata --no-deps --format-version 1 --locked --offline`.
Expected: `magic-wallstreetcn-rs` appears as a workspace member without a new
registry resolution.

- [ ] **Step 2: Write request and transport red tests**

In `transport.rs`, add tests named with `request_` and `transport_` prefixes
that require:

```rust
assert_eq!(RSS_URL, "https://dedicated.wallstreetcn.com/rss.xml");
assert_eq!(build_request().url(), RSS_URL);
assert_eq!(
    build_request().headers(),
    &[
        (
            "Accept".into(),
            "application/rss+xml, application/xml;q=0.9, text/xml;q=0.8, text/html;q=0.1".into(),
        ),
        ("User-Agent".into(), "magic-wallstreetcn-rs/0.2".into()),
    ]
);
```

Require timeout values 1 and 60 seconds to pass and 0 and 61 seconds to fail.
Require caller limits 1 and 50 to pass and 51 to fail. Test exact URL
rejection for HTTP, alternate hosts, credentials, port, query, fragment,
double slash, suffix paths, and control characters.

Test that final URL must equal `RSS_URL`, body length must be at most
2 MiB, and the MIME base type must be one of:

```rust
[
    "application/rss+xml",
    "application/xml",
    "text/xml",
    "text/html",
]
```

Require parameters and ASCII case to be normalized, but reject absent MIME,
`application/json`, and `text/plain`.

- [ ] **Step 3: Run the transport tests red**

Run:

```bash
cargo test -p magic-wallstreetcn-rs request_ --locked --offline
cargo test -p magic-wallstreetcn-rs transport_ --locked --offline
```

Expected: compilation fails because request, response, and transport types are
not implemented.

- [ ] **Step 4: Define public errors and transport seam**

In `lib.rs`, expose:

```rust
#![forbid(unsafe_code)]
//! Bounded metadata-only adapter for the WallstreetCN RSS feed.

mod transport;

pub use transport::{
    HttpRequest, HttpResponse, WallstreetCnTransport, RSS_URL,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WallstreetCnError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("WallstreetCN RSS decoding failed: {0}")]
    Decode(String),
    #[error("WallstreetCN RSS protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}
```

In `transport.rs`, define:

```rust
pub const RSS_URL: &str = "https://dedicated.wallstreetcn.com/rss.xml";
pub(crate) const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    url: String,
    headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    final_url: String,
    content_type: Option<String>,
    body: Vec<u8>,
}

pub trait WallstreetCnTransport: Send + Sync {
    fn get(
        &self,
        request: &HttpRequest,
    ) -> Result<HttpResponse, WallstreetCnError>;
}
```

Keep fields private and provide these read-only fixture/access APIs:

```rust
impl HttpRequest {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }
}

impl HttpResponse {
    pub fn new(
        final_url: impl Into<String>,
        content_type: Option<String>,
        body: Vec<u8>,
    ) -> Self {
        Self {
            final_url: final_url.into(),
            content_type,
            body,
        }
    }

    pub fn final_url(&self) -> &str {
        &self.final_url
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

pub(crate) fn build_request() -> HttpRequest {
    HttpRequest {
        url: RSS_URL.to_owned(),
        headers: vec![
            (
                "Accept".into(),
                "application/rss+xml, application/xml;q=0.9, text/xml;q=0.8, text/html;q=0.1"
                    .into(),
            ),
            (
                "User-Agent".into(),
                "magic-wallstreetcn-rs/0.2".into(),
            ),
        ],
    }
}
```

- [ ] **Step 5: Implement exact production transport**

Implement `HttpsTransport` with:

```rust
ureq::AgentBuilder::new()
    .timeout_connect(timeout)
    .timeout_read(timeout)
    .timeout_write(timeout)
    .redirects(0)
    .build()
```

Use crate-private production entry points so `lib.rs` can construct and
revalidate transport results without exposing implementation details:

```rust
pub(crate) struct HttpsTransport {
    agent: ureq::Agent,
}

impl HttpsTransport {
    pub(crate) fn new(
        timeout: Duration,
    ) -> Result<Self, WallstreetCnError>;
}

pub(crate) fn validate_response(
    response: &HttpResponse,
) -> Result<(), WallstreetCnError>;
```

Before the call, require the request URL to equal `RSS_URL`. After the call:

1. map network/TLS/timeout errors to `Transport`;
2. map `ureq::Error::Status` and non-2xx status to `Protocol`;
3. require `response.get_url() == RSS_URL`;
4. validate the MIME base type from the closed set;
5. read with `.take((MAX_RESPONSE_BYTES + 1) as u64)`;
6. reject an empty body and a body above 2 MiB.

The `text/html` allowance is valid only because the request and final URL are
both exact; Task 3 still requires a complete RSS document.

- [ ] **Step 6: Implement client construction and clone-shared pacing**

In `lib.rs`, define:

```rust
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RETURNED_ITEMS: u32 = 50;
pub const GLOBAL_NEWS_ADMITTED: bool = false;

#[derive(Clone)]
pub struct WallstreetCnClient {
    transport: Arc<dyn WallstreetCnTransport>,
    minimum_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
}
```

Provide:

```rust
pub fn new() -> Result<Self, WallstreetCnError>;
pub fn with_timeout(timeout: Duration) -> Result<Self, WallstreetCnError>;
pub fn with_transport(
    transport: impl WallstreetCnTransport + 'static,
) -> Self;
```

Use these exact validation domains:

```rust
fn validate_timeout(timeout: Duration) -> Result<(), WallstreetCnError> {
    if (Duration::from_secs(1)..=Duration::from_secs(60)).contains(&timeout) {
        Ok(())
    } else {
        Err(WallstreetCnError::InvalidRequest(
            "timeout must be between 1 and 60 seconds".into(),
        ))
    }
}

fn validate_returned_limit(limit: u32) -> Result<(), WallstreetCnError> {
    if (1..=MAX_RETURNED_ITEMS).contains(&limit) {
        Ok(())
    } else {
        Err(WallstreetCnError::InvalidRequest(format!(
            "WallstreetCN global-news limit must be between 1 and {MAX_RETURNED_ITEMS}"
        )))
    }
}
```

`execute` locks the clone-shared gate, sleeps until one second after the prior
start, sets the new start immediately before `transport.get`, holds the lock
through the complete transport call, then validates the injected response
again. Map a poisoned gate to `Transport`.

- [ ] **Step 7: Prove pacing and injected-response revalidation**

Add deterministic tests using atomics, a `Condvar`, and cloned clients:

- the second transport call cannot begin before the first completes;
- its start is at least the configured interval after the first start;
- injected responses cannot bypass final URL, MIME, empty-body, or body-size
  checks;
- transport status errors remain `Protocol`, while network errors remain
  `Transport`.

Use a zero interval only in private unit-test construction for parser tests.

- [ ] **Step 8: Run focused transport verification**

Run:

```bash
cargo fmt --all
cargo check -p magic-wallstreetcn-rs --locked --offline
cargo test -p magic-wallstreetcn-rs request_ --locked --offline
cargo test -p magic-wallstreetcn-rs transport_ --locked --offline
cargo clippy -p magic-wallstreetcn-rs --all-targets --locked --offline -- -D warnings
git diff --check
```

Expected: all commands pass with no warnings.

- [ ] **Step 9: Commit the bounded transport**

```bash
git add Cargo.toml Cargo.lock crates/magic-wallstreetcn-rs
git commit -m "feat(wallstreetcn): add bounded RSS transport"
```

### Task 3: Strict Metadata-Only RSS Parser and News Contract

**Files:**
- Modify: `crates/magic-wallstreetcn-rs/src/lib.rs`
- Create: `crates/magic-wallstreetcn-rs/src/rss.rs`
- Create: `crates/magic-wallstreetcn-rs/tests/capabilities.rs`

- [ ] **Step 1: Write a synthetic valid RSS fixture test**

Use only synthetic titles and the sentinel strings
`NEVER_EXPOSE_DESCRIPTION` and `NEVER_EXPOSE_BODY`. The fixture must contain
the exact channel identity and two newest-first items:

```xml
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:content="urn:synthetic-content">
  <channel>
    <title>华尔街见闻</title>
    <link>https://wallstreetcn.com</link>
    <description></description>
    <language>zh-hans</language>
    <item>
      <title><![CDATA[ 合成财经标题一 ]]></title>
      <link>https://wallstreetcn.com/articles/3779002</link>
      <description><![CDATA[ NEVER_EXPOSE_DESCRIPTION ]]></description>
      <content:encoded><![CDATA[ NEVER_EXPOSE_BODY ]]></content:encoded>
      <source>华尔街见闻</source>
      <pubDate>Sun, 26 Jul 2026 10:30:00 +0800</pubDate>
    </item>
    <item>
      <title>合成财经标题二</title>
      <link>https://wallstreetcn.com/articles/3779001</link>
      <source>华尔街见闻</source>
      <pubDate>Sun, 26 Jul 2026 10:20:00 +0800</pubDate>
    </item>
  </channel>
</rss>
```

Assert:

```rust
assert_eq!(batch.records().len(), 2);
assert_eq!(first.item_id.as_str(), "3779002");
assert_eq!(first.title.as_str(), "合成财经标题一");
assert_eq!(first.publisher.as_str(), "华尔街见闻");
assert_eq!(
    first.canonical_url.as_str(),
    "https://wallstreetcn.com/articles/3779002"
);
assert_eq!(first.published_at.as_str(), "2026-07-26T10:30:00+08:00");
assert!(first.summary.is_none());
assert!(first.content.is_none());
assert!(first.instruments.is_empty());
assert_eq!(first.topics[0].as_str(), "华尔街见闻");
assert_eq!(first.language.as_str(), "zh-CN");
assert_eq!(first.evidence.provider(), ProviderId::WallstreetCn);
assert_eq!(batch.provenance().source(), "wallstreetcn-rss-v1");
assert_eq!(
    batch.provenance().batch_id(),
    Some(first.evidence.batch_id())
);
```

Also serialize the batch to JSON and assert neither sentinel string appears.

- [ ] **Step 2: Write strict parser failure tests**

Add focused `parser_` tests for every boundary:

- empty body, whitespace body, invalid UTF-8, malformed XML, missing RSS root,
  multiple channels, missing or duplicate channel identity fields;
- wrong channel title, link, or language;
- missing, empty, duplicated, or wrong-typed item fields;
- item source not exactly `华尔街见闻`;
- HTTP article URLs, alternate/subdomain hosts, credentials, port, query,
  fragment, extra path, non-decimal IDs, empty IDs, and IDs longer than 20
  digits;
- duplicate IDs, duplicate URLs, equal IDs with distinct text spellings, bad
  RFC 2822 time, and newest-first regression;
- more than 100 source items;
- DTD, custom entity, malformed numeric entity, and forbidden XML control
  references;
- structural tags nested inside ignored `description`, media, or extension
  subtrees;
- a malformed 51st item with caller limit 50, proving complete-feed
  validation precedes truncation.

- [ ] **Step 3: Run parser tests red**

Run:

```bash
cargo test -p magic-wallstreetcn-rs parser_ --locked --offline
```

Expected: tests fail because `rss::parse_response` and the strict state machine
are absent.

- [ ] **Step 4: Implement the RSS state machine**

Add `mod rss;` to `lib.rs`. In `rss.rs`, define a structural stack and
separate channel/item fields:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    ChannelTitle,
    ChannelLink,
    ChannelLanguage,
    ItemTitle,
    ItemLink,
    ItemSource,
    ItemPubDate,
}

#[derive(Default)]
struct RawItem {
    title: Option<String>,
    link: Option<String>,
    source: Option<String>,
    published_at: Option<String>,
}

#[derive(Default)]
struct RssState {
    stack: Vec<String>,
    ignored_depth: usize,
    active_field: Option<Field>,
    active_text: String,
    channel_title: Option<String>,
    channel_link: Option<String>,
    channel_language: Option<String>,
    current_item: Option<RawItem>,
    items: Vec<RawItem>,
}
```

The state machine must:

- accept exactly `rss > channel > item`;
- require the root attribute `version="2.0"`, permit namespace declarations,
  and reject unexpected non-namespace attributes on structural/field elements;
- require exactly one channel;
- collect only the seven approved channel/item fields;
- set `ignored_depth` for `description`, media, and extension subtrees so
  their text is never decoded into an output buffer;
- reject duplicate required fields rather than overwriting them;
- validate all XML attributes even on ignored elements;
- reject incomplete elements at EOF;
- reject an empty feed and more than 100 items.

- [ ] **Step 5: Implement strict XML event handling**

Use:

```rust
let mut reader = quick_xml::Reader::from_reader(body);
reader.config_mut().trim_text(true);
reader.config_mut().check_end_names = true;
```

Handle `Start`, `Empty`, `End`, `Text`, `CData`, `GeneralRef`, `DocType`,
`Decl`, `Comment`, `PI`, and `Eof` explicitly. Reject every `DocType`. Permit
only case-insensitive UTF-8 declarations. Resolve numeric references and the
five predefined XML entities; reject custom named entities and control
characters.

When `ignored_depth > 0`, maintain nesting and validate attributes but do not
accumulate `Text` or `CData`. Validate every `GeneralRef` with the same
numeric/predefined-only rule and discard its resolved value.

- [ ] **Step 6: Implement canonical mapping**

Validate article URLs by exact prefix:

```rust
const ARTICLE_PREFIX: &str = "https://wallstreetcn.com/articles/";
let article_id = url.strip_prefix(ARTICLE_PREFIX).ok_or_else(|| {
    WallstreetCnError::Protocol(
        "WallstreetCN article URL must use the exact official HTTPS path".into(),
    )
})?;
if article_id.is_empty()
    || article_id.len() > 20
    || !article_id.bytes().all(|byte| byte.is_ascii_digit())
{
    return Err(WallstreetCnError::Protocol(
        "WallstreetCN article ID must contain 1 through 20 ASCII digits".into(),
    ));
}
```

This exact prefix plus decimal-suffix check excludes ports, credentials,
queries, fragments, and extra path segments.

Parse `pubDate` with `time::format_description::well_known::Rfc2822`, preserve
its explicit offset, and format with `Rfc3339`. Reject a row newer than its
predecessor.

For each item create:

```rust
let evidence = SourceEvidence::new(
    ProviderId::WallstreetCn,
    observed_at,
    &batch_id,
)?
.with_source_at(&published_at)?;

NewsItem {
    item_id: NonEmptyText::new(article_id)?,
    title: NonEmptyText::new(title)?,
    summary: None,
    content: None,
    publisher: NonEmptyText::new("华尔街见闻")?,
    canonical_url: HttpsUrl::new(canonical_url)?,
    published_at: NonEmptyText::new(published_at)?,
    instruments: Vec::new(),
    topics: vec![NonEmptyText::new("华尔街见闻")?],
    language: NonEmptyText::new("zh-CN")?,
    evidence,
}
```

Use batch ID `wallstreetcn:{observed_at}` and strict provenance:

```rust
Provenance::new("wallstreetcn-rss-v1", observed_at)?
    .with_source_at(latest_source_at)?
    .with_batch_id(batch_id)?
```

Only after every item validates, truncate to the caller limit and return
`DataBatch::strict`.

Expose the parser only within the crate:

```rust
pub(crate) fn parse_response(
    body: &[u8],
    limit: u32,
    observed_at: &str,
) -> Result<DataBatch<NewsItem>, WallstreetCnError>
```

- [ ] **Step 7: Implement diagnostic fetch and trait boundary**

In `lib.rs`, add:

```rust
pub const fn content_capabilities() -> ContentCapabilities {
    ContentCapabilities {
        instrument_news: false,
        global_news: GLOBAL_NEWS_ADMITTED,
        announcements: false,
        announcement_discovery: false,
        investor_questions: false,
    }
}

pub fn probe_global_news(
    &self,
    limit: PositiveU32,
) -> Result<DataBatch<NewsItem>, WallstreetCnError> {
    validate_returned_limit(limit.get())?;
    let response = self.execute(&transport::build_request())?;
    let observed_at = now()?;
    rss::parse_response(response.body(), limit.get(), &observed_at)
}
```

Implement `NewsProvider` so `instrument_news` always returns typed
`Unsupported`. While `GLOBAL_NEWS_ADMITTED` is false, `global_news` returns
typed `Unsupported` without touching transport. When the constant is true,
`global_news` delegates to `probe_global_news`.

- [ ] **Step 8: Add public capability tests**

Create an injected synthetic transport and assert:

```rust
assert_eq!(
    WallstreetCnClient::content_capabilities().global_news,
    GLOBAL_NEWS_ADMITTED
);
assert!(!WallstreetCnClient::content_capabilities().instrument_news);
```

Assert `instrument_news` never calls transport. Assert a diagnostic call
returns metadata-only records. Branch the `global_news` test on
`GLOBAL_NEWS_ADMITTED`: false must return `Unsupported` with zero calls; true
must return records with one call. Assert limit 51 fails before transport.
Require:

```rust
fn assert_provider_bounds<T>()
where
    T: NewsProvider<Error = WallstreetCnError> + Send + Sync + Clone,
{
}
```

- [ ] **Step 9: Run parser and contract verification**

Run:

```bash
cargo fmt --all
cargo test -p magic-wallstreetcn-rs parser_ --locked --offline
cargo test -p magic-wallstreetcn-rs --test capabilities --locked --offline
cargo test -p magic-wallstreetcn-rs --all-targets --locked --offline
cargo clippy -p magic-wallstreetcn-rs --all-targets --locked --offline -- -D warnings
git diff --check
```

Expected: every test and lint passes, with `GLOBAL_NEWS_ADMITTED` still false.

- [ ] **Step 10: Commit strict metadata parsing**

```bash
git add crates/magic-wallstreetcn-rs/src \
  crates/magic-wallstreetcn-rs/tests
git commit -m "feat(wallstreetcn): parse RSS news metadata"
```

### Task 4: Bounded Probes and Evidence-Gated Live Admission

**Files:**
- Create: `crates/magic-wallstreetcn-rs/examples/live_probe.rs`
- Create: `crates/magic-wallstreetcn-rs/examples/load_probe.rs`
- Modify if admitted: `crates/magic-wallstreetcn-rs/src/lib.rs`
- Modify: `crates/magic-wallstreetcn-rs/tests/capabilities.rs`
- Modify:
  `.planning/2026-07-26-wallstreetcn-rss-provider/findings.md`
- Modify:
  `.planning/2026-07-26-wallstreetcn-rss-provider/progress.md`

- [ ] **Step 1: Write live-probe configuration tests**

Define:

```rust
#[derive(Debug, PartialEq, Eq)]
struct ProbeConfig {
    limit: u32,
    headline_match: Option<String>,
}
```

`MAGIC_WALLSTREETCN_LIMIT` defaults to 20 and accepts only 1 through 50.
`MAGIC_WALLSTREETCN_MATCH` is optional but must not be empty when present.
Test defaults, boundaries 1/50, invalid 0/51/non-integer, empty match, and a
non-empty UTF-8 match.

- [ ] **Step 2: Implement the metadata-only live probe**

The probe must print:

- capability state;
- batch source, source time, fetched time, batch ID, completeness, and count;
- item ID, quoted title, publisher, canonical URL, publication time, topic,
  language, evidence provider/source/observed/batch times;
- booleans proving summary/content are absent.

It must exit nonzero if any record exposes summary/content or if the optional
case-sensitive local title match is absent. It must call only
`probe_global_news`, never fetch article pages.

- [ ] **Step 3: Write and implement the bounded load probe**

`MAGIC_WALLSTREETCN_LOAD_REQUESTS` defaults to 2 and accepts only 1 through 3.
Reuse one client, run serially, print each request start/completion and count,
and fail if any record exposes summary/content, has a non-WallstreetCN
Provider identity, or disagrees with its batch ID. Also fail if total elapsed
time is less than one second times `requests - 1`.

Test the default interval and 1/3 valid versus 0/4 invalid request counts.

- [ ] **Step 4: Run probe compilation and tests**

Run:

```bash
cargo fmt --all
cargo test -p magic-wallstreetcn-rs --all-targets --locked --offline
cargo clippy -p magic-wallstreetcn-rs --all-targets --locked --offline -- -D warnings
git diff --check
```

Expected: both examples compile and their pure configuration tests pass.

- [ ] **Step 5: Run bounded production-client admission probes**

Run one metadata-reporting probe, then the default two-request serial load
probe using one clone-shared client:

```bash
cargo run -p magic-wallstreetcn-rs --example live_probe --release --locked --offline
cargo run -p magic-wallstreetcn-rs --example load_probe --release --locked --offline
```

Expected for admission: both commands exit zero; the load probe completes two
consecutive production-client fetches through one shared gate; every batch
uses source `wallstreetcn-rss-v1`; the live probe returns 1 through 20
newest-first metadata-only rows, shows `ProviderId::WallstreetCn`, and exposes
no summary/content.

If a sandbox network restriction occurs, rerun the same release binary outside
the sandbox once. Record every exact command, timestamp, and typed result.

- [ ] **Step 6: Apply the evidence-determined capability state**

If both consecutive production calls pass, change:

```rust
pub const GLOBAL_NEWS_ADMITTED: bool = true;
```

Then run a trait-path fixture test proving `global_news` calls transport and
returns strict metadata.

If either call fails, leave the constant false. Preserve
`global_news=Unsupported` and keep only the explicitly named diagnostic path.
Do not weaken transport/parser checks and do not use fixtures as admission
evidence.

- [ ] **Step 7: Optionally verify a current title locally**

Only if the user supplies a title substring or a current acceptance headline
is already present in the bounded feed, run:

```bash
MAGIC_WALLSTREETCN_MATCH='半导体' \
  cargo run -p magic-wallstreetcn-rs --example live_probe --release --locked --offline
```

This is a local post-fetch title match, not search. A miss is explicit and
does not change parser correctness.

- [ ] **Step 8: Re-run focused verification and record evidence**

Run:

```bash
cargo test -p magic-wallstreetcn-rs --all-targets --locked --offline
cargo clippy -p magic-wallstreetcn-rs --all-targets --locked --offline -- -D warnings
cargo test -p magic-market-router --test intelligence_routing --locked --offline
git diff --check
```

Update `findings.md` with the final endpoint/MIME/row-count observations and
capability decision. Update `progress.md` with exact probe and test results.

- [ ] **Step 9: Commit probe and admission evidence**

```bash
git add crates/magic-wallstreetcn-rs/examples \
  crates/magic-wallstreetcn-rs/src/lib.rs \
  crates/magic-wallstreetcn-rs/tests/capabilities.rs \
  .planning/2026-07-26-wallstreetcn-rss-provider/findings.md \
  .planning/2026-07-26-wallstreetcn-rss-provider/progress.md
git commit -m "test(wallstreetcn): record live RSS admission"
```

### Task 5: Documentation, Compliance, Deployment, and Packaging

**Files:**
- Create: `crates/magic-wallstreetcn-rs/README.md`
- Create: `docs/integrations/wallstreetcn-rss.md`
- Modify: `README.md`
- Modify: `docs/DEPLOYMENT.md`
- Modify: `docs/business_rules.md`
- Modify: `docs/UPSTREAM.md`
- Modify: `tools/compliance/check.sh`
- Modify: `tools/release/package.sh`

- [ ] **Step 1: Register BR-022**

Add:

```markdown
## BR-022 WallstreetCN RSS metadata boundary
The WallstreetCN Provider may read only
`https://dedicated.wallstreetcn.com/rss.xml`. It may expose only title,
decimal article ID, exact canonical URL, publication time, publisher,
language, topic, and provenance. RSS descriptions, article bodies, media,
article-page fetching, undocumented APIs, authenticated content, storage,
caching, search indexing, and inferred instruments are prohibited.
`global_news` may be advertised only after two consecutive bounded
production-client live probes pass; otherwise the trait remains typed
`Unsupported` and only the explicit diagnostic path may access the feed.
```

- [ ] **Step 2: Register compliance invariants**

Update `tools/compliance/check.sh` to:

- require `docs/integrations/wallstreetcn-rss.md`;
- require `crates/magic-wallstreetcn-rs/Cargo.toml`;
- require `crates/magic-wallstreetcn-rs` in workspace members;
- require `## BR-022 `;
- add `wallstreetcn` to the Router Provider-dependency rejection regex;
- retain every existing sentinel.

- [ ] **Step 3: Package both probes**

Add:

```bash
build_probe magic-wallstreetcn-rs live_probe magic-wallstreetcn-live-probe
build_probe magic-wallstreetcn-rs load_probe magic-wallstreetcn-load-probe
```

The mechanical `build_probe` count must become 30. Update every root/deployment
binary count and tree so prose matches the script.

- [ ] **Step 4: Write Provider and integration documentation**

Both documents must state:

- one exact source URL and the currently observed `text/html` mislabel;
- the closed MIME set plus mandatory RSS structural validation;
- caller/source/body/timeout/pacing bounds;
- exact field mapping and `summary=None`, `content=None`;
- no article page, fast-news API, login, cookie, body, cache, or search;
- public capability state determined in Task 4;
- exact live/load commands and environment variables;
- typed failures and provenance source `wallstreetcn-rss-v1`;
- operator responsibility for permissions and the first-party terms links.

Use only synthetic examples; do not copy a live description or article body.

- [ ] **Step 5: Update root and deployment documentation**

Add the crate to the workspace tree and capability/source matrices. Add
`dedicated.wallstreetcn.com:443` with exact `/rss.xml` path to deployment
network requirements. Add packaged binary names and health commands. State the
actual live-admission result without upgrading fixture success into production
availability.

Update strict coverage totals only after Task 6 produces the final report.

- [ ] **Step 6: Record upstream provenance**

Document that the adapter is an independent local implementation against:

- `https://dedicated.wallstreetcn.com/rss.xml`;
- `https://wallstreetcn.com/`;
- `https://wallstreetcn.com/articles/3522782`.

State that no WallstreetCN source code, private API, login state, or article
body is included.

- [ ] **Step 7: Run documentation and registration checks**

Run:

```bash
bash -n tools/release/package.sh tools/release/preflight.sh
rg -c '^build_probe ' tools/release/package.sh
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
cargo metadata --no-deps --format-version 1 --locked --offline
cargo tree -p magic-wallstreetcn-rs --edges normal --locked --offline
cargo tree -p magic-market-router --edges normal --locked --offline
rg -n 'stock_analysis|magic-wallstreetcn-rs' crates -g Cargo.toml
git diff --check
```

Expected:

- all commands exit zero;
- probe count is exactly 30;
- WallstreetCN depends only on Core and declared registry crates;
- Router has no WallstreetCN production dependency;
- no `stock_analysis` path dependency exists.

- [ ] **Step 8: Commit release registration**

```bash
git add README.md crates/magic-wallstreetcn-rs/README.md \
  docs/integrations/wallstreetcn-rss.md docs/DEPLOYMENT.md \
  docs/business_rules.md docs/UPSTREAM.md \
  tools/compliance/check.sh tools/release/package.sh
git commit -m "docs: register WallstreetCN RSS provider"
```

### Task 6: Strict Coverage and Complete Gates A Through D

**Files:**
- Modify if required:
  `crates/magic-wallstreetcn-rs/src/{lib,transport,rss}.rs`
- Modify if required:
  `crates/magic-wallstreetcn-rs/tests/capabilities.rs`
- Modify if required:
  `crates/magic-wallstreetcn-rs/examples/{live_probe,load_probe}.rs`
- Modify: `README.md`
- Modify:
  `.planning/2026-07-26-wallstreetcn-rss-provider/{task_plan,findings,progress}.md`

- [ ] **Step 1: Run strict coverage without exclusions**

Create the output directory before running the report:

```bash
mkdir -p target/llvm-cov
cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --all-targets --locked --offline \
  --json --output-path target/llvm-cov/coverage.json
python3 tools/coverage/check_thresholds.py target/llvm-cov/coverage.json
```

Expected: overall production coverage is at least 80% and the critical source
set is at least 95%. Do not exclude the new crate or lower a threshold. If a
threshold fails, use the report to add deterministic behavioral tests for the
uncovered branch, then rerun the exact commands.

- [ ] **Step 2: Update the recorded coverage totals**

Replace the previous README numerator, denominator, percentage, and date with
the exact successful report values. Record the same values and commands in
`progress.md`.

- [ ] **Step 3: Run the complete release gate serially**

Run:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked --offline
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked --offline
cargo test --workspace --doc --locked --offline
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
bash tools/release/preflight.sh
git diff --check
```

Expected: every command exits zero. Run Cargo commands serially to avoid the
shared target-directory lock ambiguity observed during the Yonhap gate.
Preserve exact failing output in `progress.md` before applying a fix.

- [ ] **Step 4: Recheck dependency and repository boundaries**

Run:

```bash
cargo tree -p magic-wallstreetcn-rs --edges normal --locked --offline
cargo tree -p magic-market-router --edges normal --locked --offline
rg -n 'stock_analysis|magic-wallstreetcn-rs' crates -g Cargo.toml
git status --short
```

Expected: only the WallstreetCN manifest names the new crate, Router remains
provider-neutral, and no downstream path appears.

- [ ] **Step 5: Record and commit release evidence**

Mark implementation and gate phases complete, but leave final review pending.
Record the admission state, coverage totals, every release command, and any
lasting source limitation.

```bash
git add README.md \
  .planning/2026-07-26-wallstreetcn-rss-provider
git commit -m "chore: record WallstreetCN release evidence"
```

### Task 7: Independent Review and Branch Handoff

**Files:**
- Review: every file changed from design commit `c2f6348`
- Modify: only files required by review findings

- [ ] **Step 1: Request independent code review**

Use the `requesting-code-review` skill. Give the reviewer:

- approved design
  `docs/superpowers/specs/2026-07-26-wallstreetcn-rss-provider-design.md`;
- this implementation plan;
- base commit `c2f6348` and current head;
- exact live-admission result;
- exact coverage and release-gate results.

Require review of:

- metadata-only and copyright/terms boundary;
- exact endpoint/final URL/MIME behavior, especially `text/html`;
- ignored-description behavior without accumulation or output;
- DTD/entity/control-reference and XML-structure safety;
- channel identity, article URL, duplicate, order, and complete-feed checks;
- typed failure and truthful capability state;
- clone-shared pacing held through response completion;
- Provider/batch provenance agreement;
- Router dependency neutrality;
- documentation, compliance, packaging, and test completeness.

- [ ] **Step 2: Address every material finding**

For each finding:

1. reproduce it with a deterministic failing test;
2. record the failure in `progress.md`;
3. make the smallest in-scope fix;
4. rerun the focused test, Clippy, and `git diff --check`;
5. commit the coherent fix.

Do not weaken a parser, transport, provenance, admission, or rights boundary
to close a review comment.

- [ ] **Step 3: Re-run release checks after review changes**

If production code changed, rerun strict coverage and the full Task 6 gate.
If only prose changed, rerun docs links, compliance, rustfmt, and
`git diff --check`. Record the exact result.

- [ ] **Step 4: Close execution records**

Set all task-plan phases complete, add the review result and final commit IDs
to `progress.md`, and commit:

```bash
git add .planning/2026-07-26-wallstreetcn-rss-provider
git commit -m "chore: complete WallstreetCN provider review"
```

- [ ] **Step 5: Offer branch integration choices**

Use the `finishing-a-development-branch` skill. Preserve the primary checkout
and all user-owned changes. Present merge/PR/keep/discard choices with the
verified branch name and worktree path; do not integrate without the user's
choice.
