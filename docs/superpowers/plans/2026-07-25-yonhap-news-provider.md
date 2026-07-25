# Yonhap Chinese RSS News Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a provenance-preserving Yonhap simplified-Chinese RSS Provider that exposes bounded latest-news metadata without fetching, storing, or redistributing article bodies.

**Architecture:** Add a first-class Core identity and a standalone `magic-yonhap-rs` crate with a closed seven-channel model, injected transport, strict streaming RSS parser, clone-shared request gate, diagnostic fetch path, and evidence-gated `NewsProvider` admission. Keep Router production code provider-neutral and register only fixture identity, probes, documentation, packaging, and compliance.

**Tech Stack:** Rust 2021, `magic-market-core`, `magic-market-router`, `ureq` 2.12.1 with Rust TLS, `quick-xml` 0.41.0, `time` 0.3.54, deterministic injected-transport tests, Cargo release gates.

---

## Invariants

- Map only RSS title, stable Yonhap article ID, exact official URL, source time,
  channel, publisher, language, and evidence.
- Always leave `summary` and `content` as `None`; never request article pages.
- Permit only the seven documented
  `https://cn.yna.co.kr/RSS/*.xml` endpoints and canonical
  `https://cn.yna.co.kr/view/ACK<17 digits>` article URLs.
- Validate the complete feed before truncating to the caller limit.
- Reject DTDs, custom named entities, malformed or missing required fields,
  duplicate IDs or URLs, more than 100 source items, and source-order
  regressions.
- Advertise `global_news=true` only after the production Rust client passes a
  bounded live probe. A failed or unavailable live probe is recorded as an
  explicit failure; fixtures must not override it.
- Do not add any downstream path dependency or production Provider dependency
  to `magic-market-router`.

## File Structure

- Modify `Cargo.toml`: register the new workspace member.
- Modify `Cargo.lock`: lock the pinned XML and time dependencies.
- Modify `crates/magic-market-core/src/provider.rs`: add
  `ProviderId::Yonhap`.
- Modify `crates/magic-market-core/tests/provider_identity.rs`: lock the public
  identity and serialization name.
- Create `crates/magic-yonhap-rs/Cargo.toml`: standalone Provider manifest.
- Create `crates/magic-yonhap-rs/src/lib.rs`: channel model, request building,
  bounded transport, pacing, parser, mapping, Provider implementation, and
  unit tests.
- Create `crates/magic-yonhap-rs/tests/capabilities.rs`: public capability and
  typed-trait contract.
- Create `crates/magic-yonhap-rs/examples/live_probe.rs`: bounded official
  endpoint evidence and local headline match.
- Create `crates/magic-yonhap-rs/examples/load_probe.rs`: serial, at-most-three
  request pacing evidence.
- Create `crates/magic-yonhap-rs/README.md`: public API, limits, legal boundary,
  and probe commands.
- Modify `crates/magic-market-router/tests/intelligence_routing.rs`: Yonhap
  fixture identity acceptance and mismatch rejection.
- Create `docs/integrations/yonhap-rss.md`: source, mapping, admission state,
  runtime controls, and failure semantics.
- Modify `README.md`: Provider/capability matrix, workspace tree, probes,
  package contents, host requirements, and legal boundary.
- Modify `docs/DEPLOYMENT.md`: binaries, network allowlist, health commands,
  and admission state.
- Modify `docs/business_rules.md`: register BR-021.
- Modify `docs/UPSTREAM.md`: register official RSS and terms provenance.
- Modify `tools/compliance/check.sh`: require the crate, integration document,
  workspace member, BR-021, and provider-neutral Router boundary.
- Modify `tools/release/package.sh`: build and package both Yonhap probes.
- Update
  `.planning/2026-07-25-yonhap-news-provider/{task_plan,findings,progress}.md`
  after every evidence boundary.

### Task 1: Add the first-class Provider identity

**Files:**
- Modify: `crates/magic-market-core/src/provider.rs`
- Modify: `crates/magic-market-core/tests/provider_identity.rs`
- Modify: `crates/magic-market-router/tests/intelligence_routing.rs`

- [x] **Step 1: Write failing Core identity tests**

Add Yonhap to `intelligence_sources_have_first_class_identities`, change the
expected length from 11 to 12, and extend the stable serialization test:

```rust
assert_eq!(
    serde_json::to_string(&ProviderId::Yonhap).unwrap(),
    "\"Yonhap\""
);
```

- [x] **Step 2: Write the failing Router fixture test**

Add a Provider-neutral fixture test next to
`global_news_router_accepts_eastmoney_identity`:

```rust
#[test]
fn global_news_router_accepts_yonhap_identity() {
    let provider = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::Yonhap,
        batch_source: "yonhap-cn-rss-v1",
        item_count: 2,
        duplicate_id: false,
    });
    let mut router =
        GlobalNewsRouter::new(AcceptancePolicy::new().with_require_source_at(true));
    router
        .register(global_news_source(ProviderId::Yonhap, provider, classify))
        .unwrap();

    let outcome = router.route(&PositiveU32::new(2).unwrap()).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Yonhap);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Selected
    ));
}
```

Extend the existing mismatch case with a Yonhap-registered fixture whose
record evidence says `ProviderId::ThePaper`; assert the attempt is rejected as
`FailureKind::Evidence`.

- [x] **Step 3: Verify the tests fail**

Run:

```bash
cargo test -p magic-market-core --test provider_identity --locked --offline
cargo test -p magic-market-router --test intelligence_routing --locked --offline
```

Expected: compilation fails because `ProviderId::Yonhap` does not exist.

- [x] **Step 4: Add the identity**

Add `Yonhap` immediately after the other financial-news identities:

```rust
pub enum ProviderId {
    // existing variants
    Jin10,
    ThePaper,
    Yonhap,
    // remaining variants
}
```

Do not customize Serde naming; the stable wire value must remain `"Yonhap"`.

- [x] **Step 5: Run the identity tests**

Run the two commands from Step 3.

Expected: both test binaries pass.

- [x] **Step 6: Commit the identity boundary**

```bash
git add crates/magic-market-core/src/provider.rs \
  crates/magic-market-core/tests/provider_identity.rs \
  crates/magic-market-router/tests/intelligence_routing.rs
git commit -m "feat(core): add Yonhap provider identity"
```

### Task 2: Create the bounded request and transport seam

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/magic-yonhap-rs/Cargo.toml`
- Create: `crates/magic-yonhap-rs/src/lib.rs`

- [x] **Step 1: Register the crate and exact dependencies**

Add `"crates/magic-yonhap-rs"` to workspace members. Create the manifest:

```toml
[package]
name = "magic-yonhap-rs"
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

Resolve once with network access and commit the resulting lockfile. Thereafter
all deterministic checks use `--locked --offline`.

- [x] **Step 2: Write failing channel and request tests**

In `src/lib.rs`, write tests first for this exact public channel matrix:

```rust
let cases = [
    (YonhapChannel::Rolling, "https://cn.yna.co.kr/RSS/news.xml", "滚动"),
    (YonhapChannel::Politics, "https://cn.yna.co.kr/RSS/politics.xml", "政治"),
    (YonhapChannel::Economy, "https://cn.yna.co.kr/RSS/economy.xml", "经济"),
    (YonhapChannel::Society, "https://cn.yna.co.kr/RSS/society.xml", "社会"),
    (
        YonhapChannel::CultureSports,
        "https://cn.yna.co.kr/RSS/culture-sports.xml",
        "文化体育",
    ),
    (YonhapChannel::NorthKorea, "https://cn.yna.co.kr/RSS/nk.xml", "朝鲜"),
    (
        YonhapChannel::ChinaKorea,
        "https://cn.yna.co.kr/RSS/china-relationship.xml",
        "中韩关系",
    ),
];
for (channel, endpoint, topic) in cases {
    assert_eq!(channel.endpoint(), endpoint);
    assert_eq!(channel.topic(), topic);
}
```

Also require:

- `YonhapClient::new()` defaults to `Rolling`;
- timeouts of 0 and 61 seconds fail before transport;
- timeouts of 1 and 60 seconds succeed;
- a caller limit of 51 fails before transport;
- requests contain only `Accept: application/rss+xml, application/xml;q=0.9,
  text/xml;q=0.8` and `User-Agent: magic-yonhap-rs/0.2`;
- the URL validator rejects HTTP, credentials, non-443 ports, lookalike hosts,
  query strings, fragments, unknown paths, and control characters;
- XML MIME accepts `application/rss+xml`, `application/xml`, and `text/xml`
  case-insensitively with optional parameters, and rejects absent MIME, HTML,
  JSON, and `application/xmlx`.

- [x] **Step 3: Verify the new tests fail**

Run:

```bash
cargo test -p magic-yonhap-rs channel_and_request --locked --offline
```

Expected: compilation fails because the channel, client, request, and
validators are not implemented.

- [x] **Step 4: Implement the public model and typed errors**

Use these public shapes:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YonhapChannel {
    Rolling,
    Politics,
    Economy,
    Society,
    CultureSports,
    NorthKorea,
    ChinaKorea,
}

#[derive(Debug, Error)]
pub enum YonhapError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("Yonhap RSS decoding failed: {0}")]
    Decode(String),
    #[error("Yonhap RSS protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    url: String,
    headers: Vec<(String, String)>,
}

pub trait YonhapTransport: Send + Sync {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, YonhapError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    final_url: String,
    content_type: Option<String>,
    body: Vec<u8>,
}
```

Expose read-only accessors for request URL/headers and response fields so
injected transports and probes can inspect evidence without mutating it.
`YonhapChannel::endpoint`, `topic`, and `slug` must be closed `match`
expressions; do not construct paths from caller text.

- [x] **Step 5: Implement the production transport and shared gate**

Use constants:

```rust
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_RETURNED_ITEMS: u32 = 50;
const MAX_SOURCE_ITEMS: usize = 100;
```

`HttpsTransport` must:

1. validate the requested URL against the exact channel endpoints;
2. use `ureq::AgentBuilder` timeouts and `.redirects(0)`;
3. require HTTP 200;
4. validate the final response URL again;
5. require an XML-compatible `Content-Type`;
6. read through `.take((MAX_RESPONSE_BYTES + 1) as u64)`;
7. fail with `Protocol` if the body exceeds 2 MiB.

`YonhapClient` stores:

```rust
pub struct YonhapClient {
    channel: YonhapChannel,
    transport: Arc<dyn YonhapTransport>,
    minimum_interval: Duration,
    request_gate: Arc<Mutex<Option<Instant>>>,
}
```

Provide `new`, `for_channel`, `with_timeout`, and `with_transport`. All clones
share `request_gate`. Acquire the mutex before waiting, record the request
start, keep the guard through `transport.get`, and translate a poisoned lock
to `YonhapError::Transport`.

- [x] **Step 6: Add deterministic transport-bound tests**

Use injected transports and a controllable zero-duration private constructor
for unit tests. Require:

- invalid limit and timeout cause zero transport calls;
- oversize injected bodies are rejected even if a custom transport bypasses
  the production reader;
- wrong final URL, wrong MIME, and non-200 production responses remain
  distinguishable failures;
- two clones serialize starts and the second starts at least the configured
  interval after the first;
- the gate remains held while the first response body is being returned.

Do not weaken the public one-second minimum; only private test construction may
use shorter intervals.

- [x] **Step 7: Resolve dependencies and run the transport tests**

Run:

```bash
cargo check -p magic-yonhap-rs
cargo test -p magic-yonhap-rs channel_and_request --locked --offline
cargo test -p magic-yonhap-rs transport --locked --offline
```

Expected: Cargo.lock contains `quick-xml 0.41.0` and `time 0.3.54`; all new
tests pass.

- [x] **Step 8: Commit the transport boundary**

```bash
git add Cargo.toml Cargo.lock crates/magic-yonhap-rs/Cargo.toml \
  crates/magic-yonhap-rs/src/lib.rs
git commit -m "feat(yonhap): add bounded RSS transport"
```

### Task 3: Implement strict RSS parsing and metadata-only mapping

**Files:**
- Modify: `crates/magic-yonhap-rs/src/lib.rs`
- Create: `crates/magic-yonhap-rs/tests/capabilities.rs`

- [x] **Step 1: Add the valid RSS fixture test**

Use a complete UTF-8 RSS fixture with at least three newest-first items. Include
an escaped XML entity in a title, a `description` CDATA payload, a
`content:encoded` payload, a GUID URL, and RFC 2822 timestamps. Require:

```rust
let batch = client
    .probe_global_news(PositiveU32::new(2).unwrap())
    .unwrap();
assert_eq!(batch.records().len(), 2);
let first = &batch.records()[0];
assert_eq!(first.item_id.as_str(), "ACK20260725001100881");
assert_eq!(first.title.as_str(), "韩国与美国扩大芯片合作");
assert_eq!(first.publisher.as_str(), "韩联社");
assert_eq!(
    first.canonical_url.as_str(),
    "https://cn.yna.co.kr/view/ACK20260725001100881"
);
assert_eq!(first.published_at.as_str(), "2026-07-25T15:35:00+09:00");
assert!(first.summary.is_none());
assert!(first.content.is_none());
assert!(first.instruments.is_empty());
assert_eq!(first.topics[0].as_str(), "经济");
assert_eq!(first.language.as_str(), "zh-CN");
assert_eq!(first.evidence.provider(), ProviderId::Yonhap);
assert_eq!(
    first.evidence.source_at(),
    Some("2026-07-25T15:35:00+09:00")
);
assert_eq!(
    batch.provenance().source(),
    "yonhap-cn-rss-v1"
);
assert_eq!(
    batch.provenance().source_at(),
    Some("2026-07-25T15:35:00+09:00")
);
assert!(batch.quality().is_complete());
```

Assert separately that no unique phrase from either ignored body element
appears in any normalized field.

- [x] **Step 2: Add the parser failure matrix**

Add one focused test per failure:

- empty body and whitespace-only body;
- malformed XML and truncated XML;
- any `DOCTYPE`;
- a custom named entity declaration/reference;
- invalid UTF-8;
- wrong/missing `rss`, `channel`, or `item` structure;
- zero items and 101 items;
- missing/empty title, link, or publication time;
- title containing a control character after decoding;
- HTTP article URL, alternate host, credentials, query, fragment, port, wrong
  path, non-`ACK` prefix, or an ID not containing exactly 17 ASCII digits;
- present GUID that does not identify the same canonical article;
- invalid RFC 2822 timestamp;
- duplicate article ID or canonical URL;
- older row followed by a newer row;
- a limit of 1 still rejects an invalid third source row, proving validation
  occurs before truncation.

Allow only the five predefined XML named entities and numeric character
references. Reject every other `Event::GeneralRef`; reject every
`Event::DocType`. Do not enable DTD expansion.

- [x] **Step 3: Verify parser tests fail**

Run:

```bash
cargo test -p magic-yonhap-rs parser --locked --offline
```

Expected: the diagnostic fetch/parser path is missing or returns an
unimplemented error.

- [x] **Step 4: Implement streaming parsing**

Implement:

```rust
fn parse_response(
    body: &[u8],
    channel: YonhapChannel,
    limit: u32,
    observed_at: &str,
) -> Result<DataBatch<NewsItem>, YonhapError>
```

Configure `quick_xml::Reader` to trim text. Track the exact RSS/channel/item
nesting and collect only direct item children `title`, `link`, `guid`, and
`pubDate`. Consume but never retain `description`, `content:encoded`, media,
or unknown extension contents.

For text fields:

- decode UTF-8 strictly;
- resolve only predefined XML entities and numeric character references;
- collapse Unicode whitespace to single ASCII spaces;
- reject empty or control-bearing required values.

For each item:

1. validate the exact article URL;
2. extract the `ACK` plus 17-digit ID;
3. if GUID is present, accept only the same ID or exact canonical URL;
4. parse `pubDate` with `time::format_description::well_known::Rfc2822`;
5. convert to `UtcOffset::from_hms(9, 0, 0)` and format with `Rfc3339`;
6. enforce non-increasing normalized source time;
7. enforce unique ID and URL.

After validating all 1–100 source items, truncate to the caller limit and
construct:

```rust
let batch_id = format!("yonhap:{observed_at}:{}", channel.slug());
let evidence = SourceEvidence::new(
    ProviderId::Yonhap,
    observed_at,
    &batch_id,
)?
.with_source_at(&published_at)?;
let item = NewsItem {
    item_id: NonEmptyText::new(article_id)?,
    title: NonEmptyText::new(title)?,
    summary: None,
    content: None,
    publisher: NonEmptyText::new("韩联社")?,
    canonical_url: HttpsUrl::new(canonical_url)?,
    published_at: NonEmptyText::new(published_at)?,
    instruments: Vec::new(),
    topics: vec![NonEmptyText::new(channel.topic())?],
    language: NonEmptyText::new("zh-CN")?,
    evidence,
};
```

Create strict batch provenance with source `yonhap-cn-rss-v1`, fetched time
`observed_at`, latest selected source time, and the same `batch_id`.

- [x] **Step 5: Implement the diagnostic and trait boundary**

Add:

```rust
pub fn probe_global_news(
    &self,
    limit: PositiveU32,
) -> Result<DataBatch<NewsItem>, YonhapError>
```

It validates `limit <= 50`, builds the closed channel request, executes the
shared gate, applies response bounds again after injected transport, gets a
checked local observation timestamp, and calls `parse_response`.

Before live admission, keep:

```rust
pub const fn content_capabilities() -> ContentCapabilities {
    ContentCapabilities {
        instrument_news: false,
        global_news: false,
        announcements: false,
        announcement_discovery: false,
        investor_questions: false,
    }
}

impl NewsProvider for YonhapClient {
    type Error = YonhapError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(YonhapError::Unsupported(
            "Yonhap RSS does not expose a verified instrument/date filter".into(),
        ))
    }

    fn global_news(
        &self,
        _limit: PositiveU32,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(YonhapError::Unsupported(
            "Yonhap global news is pending bounded live admission; use probe_global_news for explicit diagnostics".into(),
        ))
    }
}
```

- [x] **Step 6: Add public capability tests**

`tests/capabilities.rs` must assert:

- `instrument_news` is false and returns `YonhapError::Unsupported`;
- the pre-admission `global_news` capability is false;
- the pre-admission trait call returns `YonhapError::Unsupported`;
- only `probe_global_news` invokes an injected transport;
- `YonhapClient: NewsProvider<Error = YonhapError> + Send + Sync + Clone`.

- [x] **Step 7: Run all deterministic Provider tests**

Run:

```bash
cargo fmt --all -- --check
cargo test -p magic-yonhap-rs --all-targets --locked --offline
cargo clippy -p magic-yonhap-rs --all-targets --locked --offline -- -D warnings
```

Expected: all deterministic parser, transport, capability, and trait tests
pass while public global news remains explicitly unadmitted.

- [x] **Step 8: Commit parser and pre-admission semantics**

```bash
git add crates/magic-yonhap-rs/src/lib.rs \
  crates/magic-yonhap-rs/tests/capabilities.rs
git commit -m "feat(yonhap): parse RSS news metadata"
```

### Task 4: Add probes and perform live capability admission

**Files:**
- Create: `crates/magic-yonhap-rs/examples/live_probe.rs`
- Create: `crates/magic-yonhap-rs/examples/load_probe.rs`
- Modify after evidence:
  `crates/magic-yonhap-rs/src/lib.rs`
- Modify after evidence:
  `crates/magic-yonhap-rs/tests/capabilities.rs`
- Modify:
  `.planning/2026-07-25-yonhap-news-provider/findings.md`
- Modify:
  `.planning/2026-07-25-yonhap-news-provider/progress.md`

- [x] **Step 1: Implement and unit-test environment parsing**

`live_probe` accepts:

- `MAGIC_YONHAP_CHANNEL`: `rolling`, `politics`, `economy`, `society`,
  `culture-sports`, `north-korea`, or `china-korea`; default `rolling`;
- `MAGIC_YONHAP_LIMIT`: integer 1–50; default 20;
- `MAGIC_YONHAP_MATCH`: optional non-empty case-sensitive local title
  substring.

Invalid environment input exits nonzero before a network call. The probe calls
`probe_global_news`, prints capability state and for every record prints
provider, batch ID, item ID, title, URL, timestamp, topic, and booleans proving
summary/content are absent. If `MAGIC_YONHAP_MATCH` is present but absent from
all returned titles, return an explicit failure.

Add pure parser tests for every channel spelling, limit bounds, empty match,
and unknown variables' values.

- [x] **Step 2: Implement the serial load probe**

`load_probe` accepts `MAGIC_YONHAP_LOAD_REQUESTS`, defaults to 2, and permits
only 1–3. Reuse one `YonhapClient`, call `probe_global_news` serially with a
small limit, print each start/completion and record count, and assert measured
request starts remain at least one second apart. No threads and no bypass of
the client gate are allowed.

- [x] **Step 3: Build and test both probes offline**

Run:

```bash
cargo test -p magic-yonhap-rs --all-targets --locked --offline
cargo clippy -p magic-yonhap-rs --all-targets --locked --offline -- -D warnings
```

Expected: both examples compile and their deterministic environment tests
pass.

- [x] **Step 4: Run the bounded official production probe**

Run the default and financial channel through the production Rust client:

```bash
cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline
MAGIC_YONHAP_CHANNEL=economy MAGIC_YONHAP_LIMIT=20 \
  cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline
```

For the initiating headline, run a local current-window check without claiming
historical search:

```bash
MAGIC_YONHAP_CHANNEL=economy \
MAGIC_YONHAP_MATCH='半导体' \
  cargo run -p magic-yonhap-rs --example live_probe --release --locked --offline
```

Admission requires two consecutive successful bounded fetches from at least
one official feed. Each must return at least one current metadata-only record,
exact Yonhap evidence, newest-first times, exact canonical URLs, and absent
summary/content.

- [x] **Step 5: Apply the evidence-determined capability state**

If the admission condition passes:

```rust
pub const fn content_capabilities() -> ContentCapabilities {
    ContentCapabilities {
        instrument_news: false,
        global_news: true,
        announcements: false,
        announcement_discovery: false,
        investor_questions: false,
    }
}

fn global_news(
    &self,
    limit: PositiveU32,
) -> Result<DataBatch<NewsItem>, Self::Error> {
    self.probe_global_news(limit)
}
```

Update capability tests to require `global_news=true` and prove the trait uses
the injected fixture transport.

If DNS, TLS, HTTP, MIME, parsing, or provenance evidence fails, make no code
change to the pre-admission implementation: retain `global_news=false`, typed
`Unsupported`, and the diagnostic method. Record the exact command, timestamp,
endpoint, and typed failure in `findings.md` and `progress.md`.

This is a deterministic decision branch, not an implementation choice:
successful evidence admits the trait; every other result leaves it unadmitted.

- [x] **Step 6: Re-run capability and all-target checks**

Run:

```bash
cargo test -p magic-yonhap-rs --all-targets --locked --offline
cargo clippy -p magic-yonhap-rs --all-targets --locked --offline -- -D warnings
git diff --check
```

Expected: tests match the evidence-determined public capability exactly.

- [x] **Step 7: Commit probes and admission evidence**

```bash
git add crates/magic-yonhap-rs/examples \
  crates/magic-yonhap-rs/src/lib.rs \
  crates/magic-yonhap-rs/tests/capabilities.rs \
  .planning/2026-07-25-yonhap-news-provider/findings.md \
  .planning/2026-07-25-yonhap-news-provider/progress.md
git commit -m "test(yonhap): record live RSS admission"
```

### Task 5: Register documentation, policy, compliance, and packaging

**Files:**
- Create: `crates/magic-yonhap-rs/README.md`
- Create: `docs/integrations/yonhap-rss.md`
- Modify: `README.md`
- Modify: `docs/DEPLOYMENT.md`
- Modify: `docs/business_rules.md`
- Modify: `docs/UPSTREAM.md`
- Modify: `tools/compliance/check.sh`
- Modify: `tools/release/package.sh`

- [x] **Step 1: Write BR-021**

Append:

```markdown
## BR-021 Yonhap Chinese RSS metadata boundary

The Yonhap Provider may read only one of the seven official simplified-Chinese
RSS endpoints per bounded request. It maps title, exact canonical article
identity and URL, publication time, channel and provenance only; summary and
content remain absent and article pages are never fetched. The complete feed
must pass exact endpoint, XML structure, required-field, unique-ID/URL,
newest-first and 100-row bounds before caller-limit truncation. Public global
news capability is true only after the production Rust client passes bounded
live admission; otherwise the trait remains explicitly unsupported and only
the named diagnostic method may perform the fetch.
```

Change the compliance sentinel to require `BR-021`.

- [x] **Step 2: Register compliance and packaging**

In `tools/compliance/check.sh`:

- require `docs/integrations/yonhap-rss.md`;
- require `crates/magic-yonhap-rs/Cargo.toml`;
- add `crates/magic-yonhap-rs` to `workspace_members`;
- add `yonhap` to the Router Provider-dependency rejection regex;
- require both BR-020 and BR-021.

In `tools/release/package.sh`, add:

```bash
build_probe magic-yonhap-rs live_probe magic-yonhap-live-probe
build_probe magic-yonhap-rs load_probe magic-yonhap-load-probe
```

Count the actual `build_probe` calls mechanically and update every documented
package-binary count to that exact value.

- [x] **Step 3: Write Provider and integration documentation**

Both new documents must state:

- all seven exact endpoints and channel spellings;
- the evidence-determined `global_news` capability state;
- `instrument_news=false`;
- 50 returned-item, 100 source-item, 2 MiB, 1–60 second timeout, and one-second
  pacing limits;
- exact host/path/final-URL/MIME checks and no redirects;
- title/ID/URL/time/channel/publisher/language/evidence mapping;
- `summary=None`, `content=None`, empty instruments;
- no article fetch, storage, caching, indexing, translation, or historical
  search;
- `MAGIC_YONHAP_CHANNEL`, `MAGIC_YONHAP_LIMIT`,
  `MAGIC_YONHAP_MATCH`, and `MAGIC_YONHAP_LOAD_REQUESTS`;
- diagnostic versus admitted trait semantics;
- typed failure categories;
- the official RSS guide and Chinese terms links.

- [x] **Step 4: Update root and deployment documentation**

Update all relevant root README locations:

- crate capability table and workspace dependency tree;
- public Provider matrix;
- latest-news examples and commands;
- source/resource limit table;
- packaged binary tree and exact probe count;
- platform/network requirements;
- legal/source boundary;
- acceptance and integration-doc index.

Update `docs/DEPLOYMENT.md`:

- add both binary names and commands;
- permit outbound TLS only to `cn.yna.co.kr:443`;
- document the seven exact paths;
- state the evidence-determined admission status;
- add health checks that print capability and record provenance;
- state that a missing current headline is not evidence of historical absence.

Update `docs/UPSTREAM.md` with the official RSS directory and terms URL, access
date `2026-07-25`, metadata-only scope, and no copied upstream source code.

- [x] **Step 5: Run documentation and structural checks**

Run:

```bash
bash -n tools/compliance/check.sh
bash -n tools/release/package.sh
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
cargo metadata --no-deps --format-version 1 --locked --offline
git diff --check
```

Expected: every command exits zero, Cargo metadata lists
`magic-yonhap-rs`, and Router still has no Provider production dependency.

- [x] **Step 6: Commit release registration**

```bash
git add README.md crates/magic-yonhap-rs/README.md \
  docs/integrations/yonhap-rss.md docs/DEPLOYMENT.md \
  docs/business_rules.md docs/UPSTREAM.md \
  tools/compliance/check.sh tools/release/package.sh
git commit -m "docs: register Yonhap RSS provider"
```

### Task 6: Coverage and complete Gates A through D

**Files:**
- Modify if required: deterministic tests in
  `crates/magic-yonhap-rs/src/lib.rs`,
  `crates/magic-yonhap-rs/tests/capabilities.rs`, and probe modules
- Modify:
  `.planning/2026-07-25-yonhap-news-provider/{task_plan,findings,progress}.md`

- [ ] **Step 1: Run focused coverage without exclusions**

Run the repository's strict coverage workflow:

```bash
cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --all-targets --locked --offline \
  --json --output-path target/llvm-cov/coverage.json
python3 tools/coverage/check_thresholds.py target/llvm-cov/coverage.json
```

Expected: all existing thresholds, including at least 80% line coverage, pass.
Do not exclude the new crate or lower a threshold. If coverage fails, add
deterministic behavior tests for the reported uncovered branches and rerun the
same commands.

- [ ] **Step 2: Run the complete release gate**

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

Expected: every command exits zero. Preserve exact failing command output in
`progress.md` before fixing any discovered issue.

- [ ] **Step 3: Verify manifest and dependency boundaries**

Run:

```bash
cargo tree -p magic-yonhap-rs --edges normal --locked --offline
cargo tree -p magic-market-router --edges normal --locked --offline
rg -n 'stock_analysis|magic-yonhap-rs' crates/*/Cargo.toml
```

Expected:

- Yonhap depends only on Core and registry dependencies declared in its
  manifest;
- Router does not depend on `magic-yonhap-rs`;
- no downstream `stock_analysis` path appears.

- [ ] **Step 4: Record and commit gate evidence**

Set Phase 3 and Phase 4 complete in `task_plan.md`. Record live admission,
coverage, and every release-gate result in `progress.md`; record any lasting
operational limitation in `findings.md`.

```bash
git add .planning/2026-07-25-yonhap-news-provider
git commit -m "chore: record Yonhap release evidence"
```

### Task 7: Independent review and branch handoff

**Files:**
- Review: every file changed from base commit `6c47302`
- Modify: only files required by review findings

- [ ] **Step 1: Request an independent code review**

Use the `requesting-code-review` skill. Give the reviewer the approved design,
this plan, base commit `6c47302`, branch head, exact admission result, and gate
results. Require review of:

- metadata-only/copyright boundary;
- DTD/entity and URL validation;
- complete-feed-before-truncation behavior;
- typed failure and capability truthfulness;
- clone-shared pacing;
- Provider/batch provenance agreement;
- Router dependency neutrality;
- docs/package/compliance completeness.

- [ ] **Step 2: Resolve every finding**

For each finding, reproduce it with a focused test, make the smallest scoped
fix, and rerun that focused test. Do not dismiss a failure solely because a
live source is temporarily unavailable.

- [ ] **Step 3: Re-run final gates**

Run at minimum:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
bash tools/release/preflight.sh
git diff --check
git status --short
```

Expected: all checks pass and the worktree is clean.

- [ ] **Step 4: Commit review fixes and present integration choices**

Commit each coherent review fix with a scoped message. Then use the
`finishing-a-development-branch` skill to offer merge, pull-request, keep, or
discard choices. Do not modify the primary checkout or user-owned untracked
files without the user's selected integration action.
