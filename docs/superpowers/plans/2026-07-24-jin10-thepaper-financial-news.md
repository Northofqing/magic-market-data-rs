# Jin10 and The Paper Financial News Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add strict, read-only Jin10 and The Paper global financial-news providers with evidence, probes, routing acceptance, documentation, and release registration.

**Architecture:** Two independent provider crates normalize verified first-party structured data into `NewsItem`. Jin10 reads its public flash JSON API; The Paper extracts SSR JSON from finance channel 25951. Both use injected transports, official-origin allowlists, bounded bodies, shared pacing, explicit errors, deterministic fixtures, and provider-neutral router adapters.

**Tech Stack:** Rust 2021, `magic-market-core`, `serde_json`, `thiserror`, `ureq` 2.12.1, Cargo workspace tests, shell compliance/release gates.

---

## File structure

- Modify `crates/magic-market-core/src/provider.rs`: add the two provenance identities.
- Modify `crates/magic-market-core/tests/provider_identity.rs`: lock identity serde names.
- Modify `crates/magic-market-router/tests/intelligence_routing.rs`: prove generic global-news routing accepts both identities without production path dependencies.
- Create `crates/magic-jin10-rs/Cargo.toml`: provider crate manifest.
- Create `crates/magic-jin10-rs/src/lib.rs`: transport, request, parsing, normalization, errors, and deterministic unit tests.
- Create `crates/magic-jin10-rs/tests/capabilities.rs`: public capability declaration.
- Create `crates/magic-jin10-rs/examples/live_probe.rs`: bounded live evidence.
- Create `crates/magic-jin10-rs/examples/load_probe.rs`: capped sequential load evidence.
- Create `crates/magic-jin10-rs/README.md`: provider contract and commands.
- Create `crates/magic-thepaper-rs/Cargo.toml`: provider crate manifest.
- Create `crates/magic-thepaper-rs/src/lib.rs`: transport, SSR extraction, parsing, normalization, errors, and deterministic unit tests.
- Create `crates/magic-thepaper-rs/tests/capabilities.rs`: public capability declaration.
- Create `crates/magic-thepaper-rs/examples/live_probe.rs`: bounded live evidence.
- Create `crates/magic-thepaper-rs/examples/load_probe.rs`: capped sequential load evidence.
- Create `crates/magic-thepaper-rs/README.md`: provider contract and commands.
- Modify `Cargo.toml` and `Cargo.lock`: register both workspace crates using existing locked dependencies.
- Create `docs/integrations/jin10-web.md` and `docs/integrations/thepaper-web.md`: upstream boundaries, evidence, and operations.
- Modify `README.md`, `docs/DEPLOYMENT.md`, and `docs/business_rules.md`: capability matrix, probe inventory, and registered admission rule.
- Modify `tools/compliance/check.sh`: require both manifests/docs while retaining router neutrality.
- Modify `tools/release/package.sh`: package both live/load probe pairs.

### Task 1: Core identities and provider-neutral routing

**Files:**
- Modify: `crates/magic-market-core/src/provider.rs`
- Modify: `crates/magic-market-core/tests/provider_identity.rs`
- Modify: `crates/magic-market-router/tests/intelligence_routing.rs`

- [ ] **Step 1: Add failing identity tests**

Add exact assertions:

```rust
assert_eq!(
    serde_json::to_string(&ProviderId::Jin10).unwrap(),
    "\"Jin10\""
);
assert_eq!(
    serde_json::to_string(&ProviderId::ThePaper).unwrap(),
    "\"ThePaper\""
);
```

Add two global-news fixture providers whose `SourceEvidence` uses each new identity,
register them through `global_news_source`, make the first return an evidence-invalid
batch, and assert the router selects `ProviderId::ThePaper`.

- [ ] **Step 2: Run tests and verify the red state**

Run:

```bash
cargo test -p magic-market-core --test provider_identity --offline
cargo test -p magic-market-router --test intelligence_routing --offline
```

Expected: compile failure because the variants do not exist.

- [ ] **Step 3: Add the identities**

Insert these variants before exchange identities:

```rust
Jin10,
ThePaper,
```

- [ ] **Step 4: Run focused tests**

Run the two commands from Step 2. Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add crates/magic-market-core/src/provider.rs \
  crates/magic-market-core/tests/provider_identity.rs \
  crates/magic-market-router/tests/intelligence_routing.rs
git commit -m "feat(core): add Jin10 and The Paper identities"
```

### Task 2: Jin10 strict provider

**Files:**
- Create: `crates/magic-jin10-rs/Cargo.toml`
- Create: `crates/magic-jin10-rs/src/lib.rs`
- Create: `crates/magic-jin10-rs/tests/capabilities.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: Register a minimal crate and write failing capability/fixture tests**

Use this manifest:

```toml
[package]
name = "magic-jin10-rs"
version = "0.2.0"
edition.workspace = true
license.workspace = true

[dependencies]
magic-market-core = { path = "../magic-market-core", version = "=0.2.0" }
serde_json = "1"
thiserror = { workspace = true }
ureq = { version = "=2.12.1", default-features = false, features = ["tls"] }

[lints]
workspace = true
```

Add `crates/magic-jin10-rs` to workspace members. The capability test must assert only
`global_news == true`.

Unit fixtures must cover one public type-0 flash, one public type-2 article, and one
locked VIP placeholder. Assert:

```rust
assert_eq!(batch.records().len(), 2);
assert_eq!(batch.records()[0].evidence.provider(), ProviderId::Jin10);
assert_eq!(batch.records()[0].published_at.as_str(), "2026-07-24T22:40:37+08:00");
assert_eq!(
    batch.records()[0].canonical_url.as_str(),
    "https://flash.jin10.com/detail/20260724224037091800"
);
assert_eq!(
    batch.records()[1].canonical_url.as_str(),
    "https://xnews.jin10.com/details/225718"
);
```

Also add tests for duplicate IDs, non-`OK` envelopes, malformed public rows, unsupported
types, oversized limit, source attribution, important-topic mapping, newest-first order,
official URL/header construction, and shared request-gate serialization.

- [ ] **Step 2: Run the Jin10 tests and verify the red state**

Run:

```bash
cargo test -p magic-jin10-rs --all-targets --offline
```

Expected: compile failure until the client and errors are implemented.

- [ ] **Step 3: Implement transport and errors**

Define:

```rust
const ENDPOINT: &str = "https://flash-api.jin10.com/get_flash_list";
const MAX_PAGE_SIZE: u32 = 20;
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MINIMUM_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Error)]
pub enum Jin10Error {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("HTTPS transport error: {0}")]
    Transport(String),
    #[error("Jin10 response decoding failed: {0}")]
    Decode(String),
    #[error("Jin10 protocol error: {0}")]
    Protocol(String),
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    #[error("core contract error: {0}")]
    Core(#[from] magic_market_core::CoreError),
}
```

`HttpRequest` contains URL and headers. `Jin10Transport::get` is injected for tests.
Production `ureq` uses zero redirects, exact host validation, HTTP 200, JSON content
type, and a 2 MiB read cap.

- [ ] **Step 4: Implement strict normalization**

Implement:

```rust
impl NewsProvider for Jin10Client {
    type Error = Jin10Error;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        Err(Jin10Error::Unsupported(
            "Jin10 public flash does not expose a verified instrument/date filter".into(),
        ))
    }

    fn global_news(&self, limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        self.fetch_global_news(limit)
    }
}
```

The request URL is exactly `ENDPOINT?channel=-8200&vip=1`; headers include `Accept`,
`Origin`, `Referer`, `User-Agent`, `x-app-id`, and `x-version`. Parse `status == 200` or
message `OK`, reject duplicate source IDs, omit `data.lock == true`, accept only types 0
and 2, reject empty public content, use public title-or-content fallback, preserve source
attribution, build verified canonical URLs, convert source time to RFC 3339 `+08:00`,
sort newest first, then truncate to the caller limit.

- [ ] **Step 5: Run Jin10 tests**

Run:

```bash
cargo test -p magic-jin10-rs --all-targets --offline
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/magic-jin10-rs
git commit -m "feat(jin10): add public financial flash provider"
```

### Task 3: Jin10 probes and documentation

**Files:**
- Create: `crates/magic-jin10-rs/examples/live_probe.rs`
- Create: `crates/magic-jin10-rs/examples/load_probe.rs`
- Create: `crates/magic-jin10-rs/README.md`
- Create: `docs/integrations/jin10-web.md`

- [ ] **Step 1: Add live probe**

Fetch five records and print provider, batch provenance, each normalized field, and:

```rust
println!("live_probe_status=passed");
```

- [ ] **Step 2: Add bounded load probe**

Use `MAGIC_JIN10_LOAD_REQUESTS`, default 2, valid range 1 through 3, concurrency 1, and
print success/failure counts plus p50/p95/p99/max latency. Unit-test the bounds.

- [ ] **Step 3: Document the exact public contract**

Document endpoint, public headers, 20-row bound, one-second pacing, VIP omission,
unsupported instrument filtering, errors, provenance, and commands:

```bash
cargo run -p magic-jin10-rs --example live_probe --release
MAGIC_JIN10_LOAD_REQUESTS=2 \
  cargo run -p magic-jin10-rs --example load_probe --release
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo test -p magic-jin10-rs --all-targets --locked --offline
```

Expected: pass.

```bash
git add crates/magic-jin10-rs docs/integrations/jin10-web.md
git commit -m "docs(jin10): add probes and integration contract"
```

### Task 4: The Paper strict provider

**Files:**
- Create: `crates/magic-thepaper-rs/Cargo.toml`
- Create: `crates/magic-thepaper-rs/src/lib.rs`
- Create: `crates/magic-thepaper-rs/tests/capabilities.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: Register a minimal crate and write failing tests**

Use the same dependencies and lints as Jin10, with package name `magic-thepaper-rs`.
Fixture HTML must contain an exact `__NEXT_DATA__` script with native rows in editorial
order, one external forward, and duplicate tags. Assert:

```rust
assert_eq!(batch.records().len(), 2);
assert_eq!(batch.records()[0].item_id.as_str(), "33654589");
assert_eq!(batch.records()[0].evidence.provider(), ProviderId::ThePaper);
assert_eq!(batch.records()[0].published_at.as_str(), "2026-07-24T20:42:11+08:00");
assert_eq!(
    batch.records()[0].canonical_url.as_str(),
    "https://www.thepaper.cn/newsDetail_forward_33654589"
);
assert_eq!(batch.records()[0].publisher.as_str(), "澎湃新闻");
```

Add negative tests for missing/duplicate `__NEXT_DATA__`, wrong page/channel ID, non-200
payload, duplicate `contId`, malformed timestamp, inconsistent native flags/link,
missing node/tag fields, empty eligible set, oversized limit, and shared pacing.

- [ ] **Step 2: Run The Paper tests and verify the red state**

Run:

```bash
cargo test -p magic-thepaper-rs --all-targets --offline
```

Expected: compile failure until the provider is implemented.

- [ ] **Step 3: Implement transport and SSR extraction**

Define ThePaper equivalents of the Jin10 error/transport types and:

```rust
const ENDPOINT: &str = "https://www.thepaper.cn/channel_25951";
const CHANNEL_ID: &str = "25951";
const MAX_PAGE_SIZE: u32 = 20;
```

Require exact official URL, zero redirects, HTTP 200, HTML content type, 2 MiB cap, and
exactly one `<script id="__NEXT_DATA__" type="application/json">...</script>` block.

- [ ] **Step 4: Implement strict native-row normalization**

Implement `NewsProvider` with explicit unsupported instrument filtering. Validate page
ID and payload code 200, reject duplicate source IDs before filtering, admit only rows
with both forward flags equal to `"0"` and empty/null link, convert positive
`pubTimeLong` milliseconds to RFC 3339 `+08:00`, add subsection then unique tags as
topics, sort source time descending, and truncate after eligibility filtering.

- [ ] **Step 5: Run The Paper tests**

Run:

```bash
cargo test -p magic-thepaper-rs --all-targets --offline
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/magic-thepaper-rs
git commit -m "feat(thepaper): add native financial news provider"
```

### Task 5: The Paper probes and documentation

**Files:**
- Create: `crates/magic-thepaper-rs/examples/live_probe.rs`
- Create: `crates/magic-thepaper-rs/examples/load_probe.rs`
- Create: `crates/magic-thepaper-rs/README.md`
- Create: `docs/integrations/thepaper-web.md`

- [ ] **Step 1: Add live and load probes**

Mirror the Jin10 output contract, using `MAGIC_THEPAPER_LOAD_REQUESTS` with default 2 and
hard cap 3. Live fetch five records; load fetch ten.

- [ ] **Step 2: Document the exact SSR/native contract**

Document channel 25951, embedded JSON extraction, native-only admission, maximum limit,
one-second pacing, unsupported instrument filtering, provenance, errors, and commands.

- [ ] **Step 3: Verify and commit**

Run:

```bash
cargo test -p magic-thepaper-rs --all-targets --locked --offline
```

Expected: pass.

```bash
git add crates/magic-thepaper-rs docs/integrations/thepaper-web.md
git commit -m "docs(thepaper): add probes and integration contract"
```

### Task 6: Workspace governance and release registration

**Files:**
- Modify: `README.md`
- Modify: `docs/DEPLOYMENT.md`
- Modify: `docs/business_rules.md`
- Modify: `tools/compliance/check.sh`
- Modify: `tools/release/package.sh`

- [ ] **Step 1: Add a registered business rule**

Add `BR-012 Financial-news public access boundaries`: Jin10 admits only public unlocked
type-0/type-2 rows; The Paper admits only native channel-25951 rows; locked/forwarded
content is never relabeled or bypassed.

- [ ] **Step 2: Register capabilities and probes**

Add both sources to the README capability matrix and deployment list. Add both manifests
and integration docs to `required`, both workspace members to `workspace_members`, both
crate names to the router production-dependency denial regex, and four probe build calls
to `package.sh`.

- [ ] **Step 3: Run governance checks**

Run:

```bash
bash tools/compliance/check.sh
bash tools/docs/check_links.sh
```

Expected: both pass.

- [ ] **Step 4: Commit**

```bash
git add README.md docs/DEPLOYMENT.md docs/business_rules.md \
  tools/compliance/check.sh tools/release/package.sh
git commit -m "chore: register financial news providers"
```

### Task 7: Full verification and live evidence

**Files:**
- Modify only files required by failures directly caused by this feature.

- [ ] **Step 1: Run formatting and focused checks**

```bash
cargo fmt --all
cargo test -p magic-jin10-rs --all-targets --locked --offline
cargo test -p magic-thepaper-rs --all-targets --locked --offline
cargo test -p magic-market-core --test provider_identity --locked --offline
cargo test -p magic-market-router --test intelligence_routing --locked --offline
```

Expected: pass.

- [ ] **Step 2: Run full Gates C and D**

```bash
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked --offline
cargo test --workspace --doc --locked --offline
bash tools/compliance/check.sh
bash tools/docs/check_links.sh
git diff --check
```

Expected: every command passes.

- [ ] **Step 3: Run bounded live probes**

```bash
cargo run -p magic-jin10-rs --example live_probe --release --locked --offline
cargo run -p magic-thepaper-rs --example live_probe --release --locked --offline
```

Expected: each prints at least one normalized public record and
`live_probe_status=passed`.

- [ ] **Step 4: Review scope and commit verification fixes**

Confirm no credential, cookie, generated artifact, downstream path dependency, unrelated
user file, or planning file is staged. Commit any necessary verification-only fixes with:

```bash
git add Cargo.toml Cargo.lock README.md \
  crates/magic-market-core/src/provider.rs \
  crates/magic-market-core/tests/provider_identity.rs \
  crates/magic-market-router/tests/intelligence_routing.rs \
  crates/magic-jin10-rs crates/magic-thepaper-rs \
  docs/DEPLOYMENT.md docs/business_rules.md \
  docs/integrations/jin10-web.md docs/integrations/thepaper-web.md \
  tools/compliance/check.sh tools/release/package.sh
git commit -m "test: verify financial news providers"
```
