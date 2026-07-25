# Eastmoney Global Latest-News Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a strict, read-only Eastmoney global latest-finance-news Provider backed by the official rolling page.

**Architecture:** Add an HTML-specific method to the existing injected Eastmoney transport so JSON/PDF gates stay unchanged. Replace only `NewsProvider::global_news` with a focused rolling-page parser; keep `instrument_news` unsupported, then register capabilities, probes, routing evidence tests and documentation.

**Tech Stack:** Rust 2021, existing `ureq` transport, provider-neutral `magic-market-core::NewsProvider`, deterministic HTML fixtures, `magic-market-router`.

---

## File Structure

- `crates/magic-eastmoney-rs/src/transport.rs`: exact rolling-page host/path,
  HTML content-type and 2 MiB body gate.
- `crates/magic-eastmoney-rs/src/lib.rs`: HTML transport facade and capability.
- `crates/magic-eastmoney-rs/src/news.rs`: bounded request, complete page parser,
  `NewsItem` mapping and deterministic tests.
- `crates/magic-eastmoney-rs/tests/discovery_capabilities.rs`: public capability
  and trait assertion.
- `crates/magic-eastmoney-rs/examples/live_probe.rs`: production global-news
  smoke test.
- `crates/magic-eastmoney-rs/examples/load_probe.rs`: admitted news load mode.
- `crates/magic-market-router/tests/intelligence_routing.rs`: Eastmoney global
  news identity acceptance.
- `README.md`, `crates/magic-eastmoney-rs/README.md`,
  `docs/integrations/eastmoney-web.md`, `docs/business_rules.md`,
  `docs/DEPLOYMENT.md`: capability and operating boundary.

### Task 1: HTML-specific transport

**Files:**
- Modify: `crates/magic-eastmoney-rs/src/transport.rs`
- Modify: `crates/magic-eastmoney-rs/src/lib.rs`

- [ ] **Step 1: Write failing transport tests**

Add tests that require:

```rust
assert!(validate_news_page_endpoint(
    "https://roll.eastmoney.com/finance.html"
).is_ok());
assert!(validate_news_page_endpoint(
    "https://roll.eastmoney.com/finance_2.html"
).is_err());
assert!(validate_news_page_endpoint(
    "https://roll.eastmoney.com.example/finance.html"
).is_err());
assert!(validate_html_content_type(Some("text/html; charset=utf-8")).is_ok());
assert!(validate_html_content_type(Some("application/json")).is_err());
```

Extend the existing blocking transport test so `get_html` shares the same
request gate as `get` and `get_pdf`.

- [ ] **Step 2: Verify the tests fail**

Run:

```bash
cargo test -p magic-eastmoney-rs transport::tests --locked --offline
```

Expected: compile failure because `get_html`,
`validate_news_page_endpoint` and `validate_html_content_type` do not exist.

- [ ] **Step 3: Implement the bounded HTML path**

Add:

```rust
pub(crate) const MAX_HTML_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

pub trait EastmoneyTransport: Send + Sync {
    // existing methods
    fn get_html(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: usize,
    ) -> Result<Vec<u8>, EastmoneyError> {
        self.get(url, headers, max_bytes)
    }
}

fn validate_news_page_endpoint(url: &str) -> Result<(), EastmoneyError> {
    if url == "https://roll.eastmoney.com/finance.html" {
        Ok(())
    } else {
        Err(EastmoneyError::InvalidRequest(
            "latest news must use the exact Eastmoney finance rolling page".into(),
        ))
    }
}
```

Implement `HttpsTransport::get_html_request` by validating the exact URL,
acquiring `self.acquire_slot()`, sending the existing user agent with redirects
still disabled, requiring status 200 and
`text/html; charset=utf-8`, reading at most
`MAX_HTML_RESPONSE_BYTES + 1`, and returning
`ResponseTooLarge` when exceeded. Do not add `text/html` to
`validate_content_type`.

Expose:

```rust
pub(crate) fn get_html(
    &self,
    url: &str,
    headers: &[(&str, &str)],
) -> Result<Vec<u8>, EastmoneyError> {
    self.transport.get_html(url, headers, MAX_HTML_RESPONSE_BYTES)
}
```

- [ ] **Step 4: Run transport tests**

Run:

```bash
cargo test -p magic-eastmoney-rs transport::tests --locked --offline
```

Expected: all transport tests pass.

- [ ] **Step 5: Commit transport**

```bash
git add crates/magic-eastmoney-rs/src/transport.rs \
  crates/magic-eastmoney-rs/src/lib.rs
git commit -m "feat(eastmoney): add bounded rolling-news transport"
```

### Task 2: Global-news parser and Provider

**Files:**
- Modify: `crates/magic-eastmoney-rs/src/news.rs`

- [ ] **Step 1: Write failing Provider tests**

Use an injected HTML fixture containing at least three rows under
`<div id="artList" class="contain">`. Assert:

```rust
let batch = client.global_news(PositiveU32::new(2).unwrap()).unwrap();
assert_eq!(batch.records().len(), 2);
assert_eq!(batch.records()[0].item_id.as_str(), "202607253821086055");
assert_eq!(batch.records()[0].publisher.as_str(), "东方财富网");
assert_eq!(
    batch.records()[0].canonical_url.as_str(),
    "https://finance.eastmoney.com/a/202607253821086055.html"
);
assert_eq!(
    batch.records()[0].published_at.as_str(),
    "2026-07-25 08:40"
);
assert!(batch.records()[0].instruments.is_empty());
assert_eq!(batch.records()[0].topics[0].as_str(), "财经");
assert_eq!(batch.provenance().source_at(), Some("2026-07-25 08:40"));
```

Add separate failures for limit 21, missing `artList`, fewer rows than the
caller limit, wrong `[财经]` category, invalid dates/times, ascending source
order, duplicate article ID, title disagreement, wrong host/path and an article
ID containing nondigits.

- [ ] **Step 2: Verify the tests fail**

Run:

```bash
cargo test -p magic-eastmoney-rs news::tests --locked --offline
```

Expected: `global_news` returns `Unsupported`.

- [ ] **Step 3: Implement exact parsing**

Define:

```rust
const GLOBAL_NEWS_URL: &str = "https://roll.eastmoney.com/finance.html";
const MAX_GLOBAL_NEWS_LIMIT: u32 = 20;

impl NewsProvider for EastmoneyClient {
    type Error = EastmoneyError;

    fn global_news(
        &self,
        limit: PositiveU32,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        if limit.get() > MAX_GLOBAL_NEWS_LIMIT {
            return Err(EastmoneyError::InvalidRequest(
                "Eastmoney global-news limit must be at most 20".into(),
            ));
        }
        let body = self.get_html(
            GLOBAL_NEWS_URL,
            &[
                ("Accept", "text/html"),
                ("Referer", "https://finance.eastmoney.com/"),
            ],
        )?;
        parse_global_news(&body, limit.get() as usize)
    }
}
```

Implement a focused parser that:

1. decodes only UTF-8;
2. extracts the single `id="artList"` container;
3. parses every `<li>` with one timestamp `<span>`, exact `[财经]` category
   link and one article link;
4. calls `validate_minute_timestamp`;
5. normalizes only
   `http[s]://finance.eastmoney.com/a/<digit-id>.html` to HTTPS;
6. checks title attribute and visible text normalize to the same non-empty
   text;
7. rejects duplicate IDs/URLs and non-descending timestamps;
8. validates the complete page before truncating to `limit`;
9. creates one `BatchContext::new("global-news", newest_timestamp)` and maps
   all selected records with per-row source time.

Keep `instrument_news` unchanged and unsupported.

- [ ] **Step 4: Run Provider tests**

Run:

```bash
cargo test -p magic-eastmoney-rs news::tests --locked --offline
```

Expected: all news tests pass.

- [ ] **Step 5: Commit Provider**

```bash
git add crates/magic-eastmoney-rs/src/news.rs
git commit -m "feat(eastmoney): provide latest global finance news"
```

### Task 3: Capability, probes and router acceptance

**Files:**
- Modify: `crates/magic-eastmoney-rs/src/lib.rs`
- Modify: `crates/magic-eastmoney-rs/tests/discovery_capabilities.rs`
- Modify: `crates/magic-eastmoney-rs/examples/live_probe.rs`
- Modify: `crates/magic-eastmoney-rs/examples/load_probe.rs`
- Modify: `crates/magic-market-router/tests/intelligence_routing.rs`

- [ ] **Step 1: Write failing capability and router tests**

Require:

```rust
assert!(EastmoneyClient::content_capabilities().global_news);
assert!(!EastmoneyClient::content_capabilities().instrument_news);
```

Extend the global-news router fixture to use
`ProviderId::Eastmoney`, ensure the record and provenance batch IDs match, and
assert the Eastmoney batch is selected when an earlier source fails.

- [ ] **Step 2: Verify the tests fail**

Run:

```bash
cargo test -p magic-eastmoney-rs --test discovery_capabilities --locked --offline
cargo test -p magic-market-router --test intelligence_routing --locked --offline
```

Expected: Eastmoney `global_news` capability assertion fails.

- [ ] **Step 3: Register and expose the operation**

Set `global_news: true`. In the live probe add:

```rust
probe_batch(
    "content.global_news",
    client.global_news(PositiveU32::new(5)?),
    &mut failures,
);
```

In the load probe:

- add `"news"` to `MIXED`;
- remove `"news"` from `is_diagnostic_operation`;
- replace its request with
  `print_batch(client.global_news(small)?)`;
- update unit tests so mixed contains news and only fund-flow remains
  diagnostic.

- [ ] **Step 4: Run all affected tests**

Run:

```bash
cargo test -p magic-eastmoney-rs -p magic-market-router --all-targets \
  --locked --offline
```

Expected: all tests pass.

- [ ] **Step 5: Commit registration**

```bash
git add crates/magic-eastmoney-rs/src/lib.rs \
  crates/magic-eastmoney-rs/tests/discovery_capabilities.rs \
  crates/magic-eastmoney-rs/examples/live_probe.rs \
  crates/magic-eastmoney-rs/examples/load_probe.rs \
  crates/magic-market-router/tests/intelligence_routing.rs
git commit -m "test: admit Eastmoney global news"
```

### Task 4: Documentation, live proof and release gates

**Files:**
- Modify: `README.md`
- Modify: `crates/magic-eastmoney-rs/README.md`
- Modify: `docs/integrations/eastmoney-web.md`
- Modify: `docs/business_rules.md`
- Modify: `docs/DEPLOYMENT.md`

- [ ] **Step 1: Update capability and operating documentation**

Document:

- the exact rolling-page URL and 20-row limit;
- `global_news=true`, `instrument_news=false`;
- title-only global records, minute timestamps and empty structured
  instruments;
- HTML-specific 2 MiB gate and shared one-second pacing;
- no article-body crawl, JavaScript quick-news API, cookie or account;
- the live and load commands.

Add BR-020 requiring exact official path, newest-first unique rows, complete
page validation before truncation and no inferred stock identity. Update the
compliance rule sentinel from BR-019 to BR-020.

- [ ] **Step 2: Run the real production trait**

Run:

```bash
MAGIC_EASTMONEY_LIVE_OPERATION=global-news \
cargo run -p magic-eastmoney-rs --example live_probe --release --locked --offline
```

The live probe must print at least one record, a current source timestamp,
`ProviderId::Eastmoney`, a non-empty batch ID and only canonical HTTPS
`finance.eastmoney.com/a/<id>.html` links.

- [ ] **Step 3: Run repository release gates**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked --offline
cargo test --workspace --doc --locked --offline
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
git diff --check
```

Expected: every command exits zero.

- [ ] **Step 4: Commit documentation and proof registration**

```bash
git add README.md crates/magic-eastmoney-rs/README.md \
  docs/integrations/eastmoney-web.md docs/business_rules.md \
  docs/DEPLOYMENT.md tools/compliance/check.sh \
  docs/superpowers/plans/2026-07-25-eastmoney-global-news.md
git commit -m "docs: register Eastmoney latest news"
```
