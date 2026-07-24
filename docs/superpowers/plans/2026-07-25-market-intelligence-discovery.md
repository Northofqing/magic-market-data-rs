# Market-Intelligence Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add full-market dated dragon-tiger discovery plus normalized TDX board directory, constituent and reverse-membership capabilities.

**Architecture:** New focused Core discovery contracts reuse `DragonTigerEntry` and `BoardMembership`. Eastmoney implements complete dated dragon-tiger discovery through exact datacenter pagination; a new injectable `TdxBoardProvider` normalizes existing block files. Dedicated router adapters enforce request identity, bounds, uniqueness and evidence before failover can select a batch.

**Tech Stack:** Rust 2021, `serde`, `serde_json`, existing `ureq` Eastmoney transport, existing TDX binary protocol client, provider-neutral `magic-market-core` and `magic-market-router`.

---

## File Structure

- `crates/magic-market-core/src/discovery.rs`: discovery records, requests, capabilities
  and provider traits.
- `crates/magic-market-core/src/lib.rs`: public discovery exports.
- `crates/magic-market-core/tests/discovery.rs`: request/record validation and serde tests.
- `crates/magic-market-router/src/discovery.rs`: source adapters and router aliases.
- `crates/magic-market-router/src/lib.rs`: public discovery-router exports.
- `crates/magic-market-router/src/adapters.rs`: strengthen existing reverse-membership
  adapter only.
- `crates/magic-market-router/tests/discovery_routing.rs`: failover/admission tests.
- `crates/magic-eastmoney-rs/src/datacenter_api.rs`: exact-coverage datacenter reader.
- `crates/magic-eastmoney-rs/src/discovery.rs`: dated all-market dragon-tiger Provider.
- `crates/magic-eastmoney-rs/src/dragon_tiger.rs`: share strict source-ID mapping.
- `crates/magic-eastmoney-rs/src/lib.rs`: module and capability export.
- `crates/magic-eastmoney-rs/tests/discovery_capabilities.rs`: advertised capability test.
- `crates/magic-tdx-rs/src/board_provider.rs`: injectable normalized TDX board Provider.
- `crates/magic-tdx-rs/src/lib.rs`: public Provider/source exports.
- `crates/magic-tdx-rs/tests/board_provider.rs`: deterministic provider contract tests.
- Provider examples/docs/governance files: bounded probes and release registration.

### Task 1: Core discovery contracts

**Files:**
- Create: `crates/magic-market-core/src/discovery.rs`
- Modify: `crates/magic-market-core/src/lib.rs`
- Create: `crates/magic-market-core/tests/discovery.rs`
- Modify: `crates/magic-market-core/tests/sourced_record.rs`

- [ ] **Step 1: Write failing Core contract tests**

Add tests equivalent to:

```rust
use magic_market_core::{
    BoardCategory, BoardConstituentRequest, BoardDefinition, BoardDirectoryRequest,
    DragonTigerDiscoveryRequest, Exchange, IsoDate, MarketDiscoveryCapabilities, NonEmptyText,
    PositiveU32, ProviderId, SourceEvidence, SourcedRecord,
};

#[test]
fn discovery_requests_are_explicit_and_bounded() {
    let date = IsoDate::new("2026-07-24").unwrap();
    let request = DragonTigerDiscoveryRequest::new(
        date.clone(),
        PositiveU32::new(10_000).unwrap(),
    )
    .unwrap()
    .with_exchange(Exchange::Beijing);
    assert_eq!(request.trading_date(), &date);
    assert_eq!(request.exchange(), Some(Exchange::Beijing));
    assert!(DragonTigerDiscoveryRequest::new(
        date,
        PositiveU32::new(10_001).unwrap(),
    )
    .is_err());

    let directory = BoardDirectoryRequest::new(
        BoardCategory::Concept,
        PositiveU32::new(200).unwrap(),
    )
    .unwrap();
    assert_eq!(directory.category(), BoardCategory::Concept);

    let constituents = BoardConstituentRequest::new(
        NonEmptyText::new("tdx:concept:人工智能").unwrap(),
        PositiveU32::new(400).unwrap(),
    )
    .unwrap();
    assert_eq!(constituents.board_code().as_str(), "tdx:concept:人工智能");
}

#[test]
fn board_definition_is_sourced_and_serde_checked() {
    let evidence = SourceEvidence::new(ProviderId::Tdx, "observed", "batch").unwrap();
    let board = BoardDefinition::new(
        NonEmptyText::new("tdx:industry:电力").unwrap(),
        NonEmptyText::new("电力").unwrap(),
        BoardCategory::Industry,
        PositiveU32::new(42).unwrap(),
        evidence,
    )
    .unwrap();
    assert_eq!(board.provider_id(), ProviderId::Tdx);
    assert_eq!(board.member_count().get(), 42);
    let json = serde_json::to_string(&board).unwrap();
    assert_eq!(serde_json::from_str::<BoardDefinition>(&json).unwrap(), board);
}

#[test]
fn discovery_capabilities_default_to_false() {
    assert_eq!(MarketDiscoveryCapabilities::default(), MarketDiscoveryCapabilities {
        dragon_tiger_discovery: false,
        board_directory: false,
        board_memberships: false,
        board_constituents: false,
    });
}
```

Also add `BoardDefinition` to the compile-time `SourcedRecord` assertion.

- [ ] **Step 2: Run the Core tests and verify the red state**

Run:

```bash
cargo test -p magic-market-core --test discovery --locked --offline
```

Expected: compile failure for missing discovery types.

- [ ] **Step 3: Implement the focused discovery module**

Define:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(try_from = "BoardDefinitionWire")]
pub struct BoardDefinition {
    board_code: NonEmptyText,
    board_name: NonEmptyText,
    category: BoardCategory,
    member_count: PositiveU32,
    evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DragonTigerDiscoveryRequest {
    trading_date: IsoDate,
    exchange: Option<Exchange>,
    limit: PositiveU32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardDirectoryRequest {
    category: BoardCategory,
    limit: PositiveU32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardConstituentRequest {
    board_code: NonEmptyText,
    limit: PositiveU32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MarketDiscoveryCapabilities {
    pub dragon_tiger_discovery: bool,
    pub board_directory: bool,
    pub board_memberships: bool,
    pub board_constituents: bool,
}
```

Give every request a checked constructor with a `10_000` maximum and read-only accessors.
Give `BoardDefinition` a checked constructor/accessors, checked deserialization and
`SourcedRecord`. Its private `BoardDefinitionWire` mirrors all five fields, and
`TryFrom<BoardDefinitionWire>` calls `BoardDefinition::new` so deserialization cannot
bypass constructor invariants. Define `DragonTigerDiscovery`, `BoardDirectoryProvider`
and `BoardConstituentProvider` with the complete signatures shown in the design document.

Export all public types/traits from `lib.rs`.

- [ ] **Step 4: Run Core tests**

Run:

```bash
cargo test -p magic-market-core --all-targets --locked --offline
```

Expected: all Core tests pass.

- [ ] **Step 5: Commit Core contracts**

```bash
git add crates/magic-market-core
git commit -m "feat(core): add market discovery contracts"
```

### Task 2: Provider-neutral discovery routing

**Files:**
- Create: `crates/magic-market-router/src/discovery.rs`
- Modify: `crates/magic-market-router/src/lib.rs`
- Modify: `crates/magic-market-router/src/adapters.rs`
- Create: `crates/magic-market-router/tests/discovery_routing.rs`

- [ ] **Step 1: Write failing discovery-router tests**

Create fixture Providers and cover these concrete cases:

```rust
#[test]
fn dragon_discovery_rejects_wrong_date_exchange_duplicates_and_limit() {
    let request = DragonTigerDiscoveryRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap()
    .with_exchange(Exchange::Beijing);

    let invalid = Arc::new(DragonFixture::wrong_exchange());
    let valid = Arc::new(DragonFixture::beijing());
    let mut router = DragonTigerDiscoveryRouter::new(
        AcceptancePolicy::new().with_require_source_at(true),
    );
    router.register(dragon_tiger_discovery_source(
        ProviderId::Eastmoney,
        invalid,
        classify,
    )).unwrap();
    router.register(dragon_tiger_discovery_source(
        ProviderId::Custom,
        valid,
        classify,
    )).unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Custom);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed { kind: FailureKind::Evidence, .. }
    ));
}

#[test]
fn board_routes_enforce_directory_category_and_constituent_identity() {
    let directory = BoardDirectoryRequest::new(
        BoardCategory::Concept,
        PositiveU32::new(2).unwrap(),
    ).unwrap();
    let constituents = BoardConstituentRequest::new(
        NonEmptyText::new("tdx:concept:人工智能").unwrap(),
        PositiveU32::new(2).unwrap(),
    ).unwrap();
    assert_router_falls_through_on_wrong_category(directory);
    assert_router_falls_through_on_wrong_board(constituents);
}
```

Add reverse-membership cases that reject records for non-requested instruments and
duplicate `(instrument, board_code)` identities.

- [ ] **Step 2: Run the router tests and verify the red state**

Run:

```bash
cargo test -p magic-market-router --test discovery_routing --locked --offline
```

Expected: compile failure for missing aliases/adapters.

- [ ] **Step 3: Implement discovery router aliases and adapters**

In `src/discovery.rs`, define:

```rust
pub type DragonTigerDiscoveryRouter =
    FailoverChain<DragonTigerDiscoveryRequest, DragonTigerEntry>;
pub type BoardDirectoryRouter =
    FailoverChain<BoardDirectoryRequest, BoardDefinition>;
pub type BoardConstituentRouter =
    FailoverChain<BoardConstituentRequest, BoardMembership>;
```

Implement `dragon_tiger_discovery_source`, `board_directory_source` and
`board_constituent_source`. Each closure validates request count/identity before returning
the batch. Use `HashSet` identities:

```rust
(record.trading_date(), record.entry_id().as_str())
(record.category(), record.board_code().as_str())
(
    record.instrument.clone(),
    record.board_code.as_str().to_owned(),
)
```

Return `SourceError::try_next(FailureKind::Evidence, ...)` for date/exchange/board
mismatches and `FailureKind::Quality` for duplicates or count overflow.

Strengthen `board_membership_source` in `adapters.rs` with a requested-instrument set and
unique `(instrument, board_code)` validation.

- [ ] **Step 4: Run router tests**

Run:

```bash
cargo test -p magic-market-router --all-targets --locked --offline
```

Expected: all router tests pass.

- [ ] **Step 5: Commit routing**

```bash
git add crates/magic-market-router
git commit -m "feat(router): route market discovery batches"
```

### Task 3: Exact Eastmoney full-market dragon-tiger discovery

**Files:**
- Modify: `crates/magic-eastmoney-rs/src/datacenter_api.rs`
- Create: `crates/magic-eastmoney-rs/src/discovery.rs`
- Modify: `crates/magic-eastmoney-rs/src/lib.rs`
- Create: `crates/magic-eastmoney-rs/tests/discovery_capabilities.rs`
- Modify: `crates/magic-eastmoney-rs/examples/live_probe.rs`
- Modify: `crates/magic-eastmoney-rs/examples/load_probe.rs`

- [ ] **Step 1: Add failing exact-pagination and discovery tests**

In `datacenter_api.rs`, extend the injected paging fixture and add:

```rust
#[test]
fn exact_reader_requires_stable_totals_and_full_coverage() {
    let client = EastmoneyClient::with_transport(ExactPagingTransport::stable(1_001));
    let rows = fetch_all_rows(
        &client,
        "RPT_DAILYBILLBOARD_DETAILSNEW",
        "(TRADE_DATE='2026-07-24')",
        "TRADE_ID",
        10_000,
    ).unwrap();
    assert_eq!(rows.len(), 1_001);

    let changed = EastmoneyClient::with_transport(ExactPagingTransport::changed_total());
    assert!(matches!(
        fetch_all_rows(
            &changed,
            "RPT_DAILYBILLBOARD_DETAILSNEW",
            "(TRADE_DATE='2026-07-24')",
            "TRADE_ID",
            10_000,
        ),
        Err(EastmoneyError::Protocol(_))
    ));
}
```

In `discovery.rs`, use a fixture with SH/SZ/BJ rows and two entries for one code/date:

```rust
#[test]
fn discovers_all_exchanges_and_keeps_multi_reason_ids_unique() {
    let client = EastmoneyClient::with_transport(DiscoveryTransport::fixture());
    let request = DragonTigerDiscoveryRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(10_000).unwrap(),
    ).unwrap();
    let batch = client.discover_dragon_tiger(&request).unwrap();
    assert_eq!(batch.records().len(), 4);
    assert_eq!(
        batch.records().iter().map(|row| row.entry_id().as_str()).collect::<HashSet<_>>().len(),
        4
    );
    assert!(batch.records().iter().any(
        |row| row.instrument().exchange() == Exchange::Beijing
    ));
}
```

Add failures for duplicate/non-integral `TRADE_ID`, suffix/code disagreement, wrong date,
changed totals, missing page data and invalid amount/net invariants.

- [ ] **Step 2: Run Eastmoney tests and verify the red state**

Run:

```bash
cargo test -p magic-eastmoney-rs --all-targets --locked --offline
```

Expected: compile/test failures for the missing exact reader and Provider.

- [ ] **Step 3: Implement exact datacenter pagination**

Extend the decoded page metadata with `count`. Add `fetch_all_rows` that:

```rust
const PAGE_SIZE: u32 = 500;
const MAX_EXACT_PAGES: u32 = 20;
```

- uses `pageSize=500`, `sortColumns=TRADE_ID`, `sortTypes=-1`;
- captures page/count totals from page one;
- rejects totals changing across pages;
- rejects `pages > 20` or `count > max_records`;
- requires each intermediate page to be non-empty;
- requires final `rows.len() == count`;
- preserves the proved code-9201 empty result.

Keep existing `fetch_rows` behavior unchanged for other families.

- [ ] **Step 4: Implement `DragonTigerDiscovery`**

Build the date filter exactly as:

```rust
format!("(TRADE_DATE='{}')", request.trading_date().as_str())
```

Map source exchange from `SECUCODE` suffix, cross-check `SECURITY_CODE`, require positive
integral `TRADE_ID`, and construct:

```rust
NonEmptyText::new(format!(
    "eastmoney:{}:{}",
    request.trading_date().as_str(),
    trade_id,
))?
```

Read the complete source day, then apply the optional exchange filter and caller limit.
All records and provenance use one batch ID and source date. A proved source-empty day may
return `DataBatch::strict(Vec::new(), provenance)` for direct callers; the generic router
will treat that as no-data failover.

Advertise:

```rust
MarketDiscoveryCapabilities {
    dragon_tiger_discovery: true,
    board_directory: false,
    board_memberships: false,
    board_constituents: false,
}
```

- [ ] **Step 5: Extend bounded probes**

Add `dragon-tiger-discovery` as an explicit live/load family. The live probe uses a
required `MAGIC_EASTMONEY_DRAGON_DATE=YYYY-MM-DD` variable and prints count, exchange
distribution, unique entry count and provenance. Load bounds remain concurrency one,
maximum three attempts and at least one second between requests.

- [ ] **Step 6: Run Eastmoney focused tests and a bounded live probe**

Run:

```bash
cargo test -p magic-eastmoney-rs --all-targets --locked --offline
MAGIC_EASTMONEY_DRAGON_DATE=2026-07-24 \
  cargo run -p magic-eastmoney-rs --example live_probe --release --locked --offline
```

Expected: deterministic tests pass; live output contains non-empty SH/SZ/BJ discovery with
unique IDs and complete provenance.

- [ ] **Step 7: Commit Eastmoney discovery**

```bash
git add crates/magic-eastmoney-rs
git commit -m "feat(eastmoney): discover full-market dragon-tiger entries"
```

### Task 4: Normalized TDX board Provider

**Files:**
- Create: `crates/magic-tdx-rs/src/board_provider.rs`
- Modify: `crates/magic-tdx-rs/src/lib.rs`
- Create: `crates/magic-tdx-rs/tests/board_provider.rs`
- Create: `crates/magic-tdx-rs/examples/board_live_probe.rs`
- Create: `crates/magic-tdx-rs/examples/board_load_probe.rs`

- [ ] **Step 1: Write failing deterministic Provider tests**

Define a fixture source containing `电力` industry and `人工智能` concept records:

```rust
#[derive(Clone)]
struct FixtureSource {
    industry: Vec<BlockRecord>,
    concept: Vec<BlockRecord>,
}

impl TdxBoardSource for FixtureSource {
    fn records(&self, block_type: BlockType) -> Result<Vec<BlockRecord>, TdxError> {
        match block_type {
            BlockType::Industry => Ok(self.industry.clone()),
            BlockType::Concept => Ok(self.concept.clone()),
            BlockType::Index => Err(TdxError::Unsupported("index blocks".into())),
        }
    }
}

#[test]
fn directory_constituents_and_reverse_memberships_are_consistent() {
    let provider = TdxBoardProvider::with_source(FixtureSource::new());
    let boards = provider.boards(&BoardDirectoryRequest::new(
        BoardCategory::Concept,
        PositiveU32::new(10).unwrap(),
    ).unwrap()).unwrap();
    assert_eq!(boards.records()[0].board_code().as_str(), "tdx:concept:人工智能");

    let members = provider.board_constituents(&BoardConstituentRequest::new(
        NonEmptyText::new("tdx:concept:人工智能").unwrap(),
        PositiveU32::new(10).unwrap(),
    ).unwrap()).unwrap();
    assert!(members.records().iter().all(
        |row| row.board_code.as_str() == "tdx:concept:人工智能"
    ));

    let requested = vec![
        InstrumentId::new(Exchange::Shenzhen, "002230", AssetClass::Equity).unwrap(),
    ];
    let reverse = provider.board_memberships(&requested).unwrap();
    assert_eq!(reverse.records()[0].instrument, requested[0]);
}
```

Add tests rejecting duplicate source pairs, duplicate requested instruments, unknown board
codes, `Region`/`Unknown` categories, index blocks, unverified prefixes and empty results.

- [ ] **Step 2: Run TDX tests and verify the red state**

Run:

```bash
cargo test -p magic-tdx-rs --test board_provider --locked --offline
```

Expected: compile failure for missing Provider/source types.

- [ ] **Step 3: Implement the injectable Provider**

Define:

```rust
pub trait TdxBoardSource: Send + Sync {
    fn records(&self, block_type: BlockType) -> Result<Vec<BlockRecord>, TdxError>;
}

pub struct TdxBoardProvider {
    source: Arc<dyn TdxBoardSource>,
}
```

Provide `new(ip, port, timeout)`, `with_default(ip)` and `with_source`. The production
source wraps `TdxBlockClient`.

Use readable reversible IDs:

```rust
fn board_code(category: BoardCategory, name: &str) -> String {
    match category {
        BoardCategory::Industry => format!("tdx:industry:{name}"),
        BoardCategory::Concept => format!("tdx:concept:{name}"),
        BoardCategory::Region | BoardCategory::Unknown => unreachable!(),
    }
}
```

Before mapping, validate unique `(blockname, code)` source pairs. Map `6` to Shanghai and
`0`/`3` to Shenzhen; require six ASCII digits and `AssetClass::Equity`. Use
`ProviderId::Tdx`, one `unix-ms:<value>` observation, one batch ID and no `source_at`.

Implement `BoardDirectoryProvider`, `BoardConstituentProvider` and the existing
`BoardMembershipProvider`. Advertise all three board fields in
`MarketDiscoveryCapabilities` and leave dragon discovery false.

- [ ] **Step 4: Add bounded board probes**

The live probe takes:

```text
MAGIC_TDX_BOARD_SERVER=<ip>
MAGIC_TDX_BOARD_NAME=<exact source board name>
MAGIC_TDX_BOARD_CATEGORY=industry|concept
```

It prints directory count, exact constituent count, one reverse-membership check and
provenance. The load probe permits at most three requests and concurrency one.

- [ ] **Step 5: Run TDX tests and bounded live probe**

Run:

```bash
cargo test -p magic-tdx-rs --all-targets --locked --offline
MAGIC_TDX_BOARD_SERVER=180.153.18.170 \
MAGIC_TDX_BOARD_NAME=人工智能 \
MAGIC_TDX_BOARD_CATEGORY=concept \
  cargo run -p magic-tdx-rs --example board_live_probe --release --locked --offline
```

Expected: tests pass and the live probe reports a non-empty directory/constituent batch
with TDX evidence. If the named board has changed, inspect the printed bounded directory
and rerun once with an exact current name; record that source change.

- [ ] **Step 6: Commit TDX boards**

```bash
git add crates/magic-tdx-rs
git commit -m "feat(tdx): normalize board memberships and constituents"
```

### Task 5: Documentation, capability registry and release packaging

**Files:**
- Modify: `crates/magic-eastmoney-rs/README.md`
- Modify: `crates/magic-tdx-rs/README.md`
- Modify: `docs/integrations/eastmoney-web.md`
- Create: `docs/integrations/tdx-boards.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/DEPLOYMENT.md`
- Modify: `docs/TDX_CAPABILITIES.md`
- Modify: `docs/business_rules.md`
- Modify: `tools/compliance/check.sh`
- Modify: `tools/release/package.sh`

- [ ] **Step 1: Register the two capabilities and operational bounds**

Document exact names and limitations:

```text
Eastmoney dragon-tiger discovery:
  explicit date, max 10,000, exact 500-row pages, SH/SZ/BJ, source TRADE_ID
TDX boards:
  industry/concept only, max two block requests, no source_at,
  provider-scoped board IDs, SH/SZ constituents only
```

Add live/load probe binaries to compliance and release packaging. Add one business rule
requiring complete declared pagination for market-wide discovery and one rule prohibiting
fabricated board codes/source timestamps.

- [ ] **Step 2: Run documentation/governance checks**

Run:

```bash
bash tools/compliance/check.sh
bash tools/docs/check_links.sh
bash -n tools/release/package.sh
```

Expected: all commands exit zero.

- [ ] **Step 3: Commit documentation and registration**

```bash
git add README.md CHANGELOG.md \
  crates/magic-eastmoney-rs/README.md crates/magic-tdx-rs/README.md \
  docs/DEPLOYMENT.md docs/TDX_CAPABILITIES.md docs/business_rules.md \
  docs/integrations/eastmoney-web.md docs/integrations/tdx-boards.md \
  tools/compliance/check.sh tools/release/package.sh
git commit -m "docs: register market discovery providers"
```

### Task 6: Full gates and independent review

**Files:**
- Modify only files required by concrete review findings.

- [ ] **Step 1: Run repository release gates**

Run in order:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked --offline
cargo test --workspace --doc --locked --offline
bash tools/compliance/check.sh
bash tools/docs/check_links.sh
git diff --check
```

Expected: every command exits zero.

- [ ] **Step 2: Request independent review**

Ask the reviewer to inspect the implementation from `7e658ae` through `HEAD`, focusing on:

```text
Critical/Important:
- full-market pagination cannot silently truncate;
- source identities/dates/exchanges cannot be guessed;
- multi-reason dragon entries remain unique;
- TDX board IDs are reversible and source timestamps remain absent;
- router adapters reject wrong identity, duplicates and over-limit results;
- public injected sources do not weaken production bounds.
```

- [ ] **Step 3: Resolve findings with regression tests**

For every concrete finding, first add a failing focused test, run it to verify the red
state, implement only the required correction, rerun focused tests, then repeat all gates.

- [ ] **Step 4: Commit review corrections**

If tracked corrections exist:

```bash
git add crates/magic-market-core/src/discovery.rs \
  crates/magic-market-core/src/lib.rs \
  crates/magic-market-core/tests/discovery.rs \
  crates/magic-market-router/src/adapters.rs \
  crates/magic-market-router/src/discovery.rs \
  crates/magic-market-router/src/lib.rs \
  crates/magic-market-router/tests/discovery_routing.rs \
  crates/magic-eastmoney-rs/src/datacenter_api.rs \
  crates/magic-eastmoney-rs/src/discovery.rs \
  crates/magic-eastmoney-rs/src/lib.rs \
  crates/magic-tdx-rs/src/board_provider.rs \
  crates/magic-tdx-rs/src/lib.rs \
  crates/magic-tdx-rs/tests/board_provider.rs
git commit -m "fix: enforce market discovery boundaries"
```

If no corrections exist, record the clean review in the persistent progress log without
creating an empty commit.

- [ ] **Step 5: Record final evidence**

Record commit SHAs, focused test counts, live probe counts/distributions, complete gate
commands and review outcome in:

```text
.planning/2026-07-25-market-discovery-global-calendars/progress.md
```

Planning files remain untracked and are never included in product commits.
