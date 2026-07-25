# Market-Intelligence Discovery Design

## Scope

This slice adds the A-share and cross-market discovery capabilities required by
selection workflows:

1. discover every dragon-tiger entry published for one trading date, optionally restricted
   to one domestic exchange;
2. list industry/concept boards, list one board's constituents, and find all memberships
   for requested instruments;
3. discover full-market announcements without a caller-known instrument;
4. read a verified set of global indices and major FX pairs;
5. read Jin10 economic-calendar releases and official China Government policy documents;
6. download the original body of an Eastmoney research-report PDF;
7. discover official CFFEX equity-index-futures delivery events;
8. capture the Eastmoney main-fund-flow ranking under a strict 15:35
   Asia/Shanghai contract.

The user's standing instruction to make implementation choices independently is the design
authorization for this specification.

## Current-State Constraint

`DragonTigerData` is instrument-scoped. Its request always contains an
`InstrumentId`, and the router rejects every record whose instrument differs from the
request. The current Eastmoney, SSE and SZSE adapters therefore cannot express daily market
discovery even though their source endpoints contain enough data.

Core already contains `BoardMembership`, `BoardMembershipProvider` and a router alias, but
no concrete Provider implements them. TDX already downloads and parses industry, concept
and index block files and can list constituents by source board name. That source-specific
capability is not normalized and carries no Core evidence.

## Sources

### Eastmoney dragon-tiger discovery

Use the existing first-party public datacenter endpoint and report:

```text
https://datacenter-web.eastmoney.com/api/data/v1/get
RPT_DAILYBILLBOARD_DETAILSNEW
```

The request filters by explicit `TRADE_DATE` and sorts by the source-unique `TRADE_ID`.
The existing HTTPS-only, no-redirect, bounded and one-request-per-second Eastmoney
transport remains the only network path.

A live 2026-07-24 query returned one declared page with 84 entries:

- 28 Shanghai;
- 47 Shenzhen;
- 9 Beijing.

The rows contained 72 unique instruments but 84 unique `TRADE_ID` values. Multiple reason
rows for one instrument/date are valid. Discovery identities therefore include
`TRADE_ID`; `{code}:{date}` is not unique enough.

### TDX boards

Use TDX's existing bounded block-file operations:

```text
block_fg.dat  industry/filter boards
block_gn.dat  concept boards
```

The first admitted slice excludes `block_zs.dat`. That file mixes index and regional
semantics, so mapping it to one Core `BoardCategory` would mislabel source data.

TDX block records provide a board name and constituent security code, but no board code or
source timestamp. Normalized board identities are therefore explicit provider-scoped
identifiers:

```text
tdx:industry:<source board name>
tdx:concept:<source board name>
```

This is a reversible identifier derived from source fields, not a guessed upstream code.
Record and batch `source_at` remain absent.

### Expanded production sources

#### Full-market announcements

CNInfo's public `new/hisAnnouncement/query` form accepts an empty `stock`,
`column=szse`, and a bounded `seDate`. A live 2026-07-24 probe returned 1,108
announcements across Shanghai and Shenzhen source codes with stable
`totalAnnouncement`, `totalpages`, `hasMore`, source IDs, publication
milliseconds, and PDF paths. The provider pages until the requested bound,
rejects unstable totals and duplicate IDs, and maps only verified A-share
equity code families.

#### Global indices and FX

Sina's credential-free HTTPS quote endpoint is used only for symbols proved by
live probes:

- indices: Dow Jones, Nasdaq Composite, S&P 500, Nikkei 225, Hang Seng, FTSE
  100;
- FX: USD/CNY, EUR/USD, USD/JPY, GBP/USD, AUD/USD, USD/CHF, USD/CAD, NZD/USD.

Index packets do not expose a source timestamp, so their evidence contains only
the local observation time. FX packets expose both date and time; the provider
requires both and retains their combined Asia/Shanghai source timestamp.

#### Economic calendar

Jin10 type-1 public flash rows are normalized as economic releases. Live rows
prove `data_id`, country, indicator name, period, previous/consensus/actual,
unit, star/importance, impact, scheduled publication time, and flash release
time. Protected rows are never returned. This family is intentionally a
bounded latest-release calendar rather than an unproved historical archive.

#### Official policy source

The China Government Network policy library
`https://sousuo.www.gov.cn/search-gov/data` is the only policy source. Requests
are bounded and may specify query text, publication range, page, and page size.
The provider retains official document ID, title, summary, issuing
organization, document number, category, publication date, and canonical
`gov.cn` URL.

#### Research PDF body

Eastmoney report metadata already proves the report `infoCode` and canonical
`https://pdf.dfcfw.com/pdf/H3_<infoCode>_1.pdf` URL. The document provider
accepts that exact identity pair, downloads a bounded body, requires the PDF
magic header, rejects empty/truncated/oversized bodies, and returns the
original bytes with provenance. It does not perform lossy PDF-to-text
conversion inside the market-data adapter.

#### Futures delivery calendar

The first production family is deliberately bounded to CFFEX equity-index
futures IF, IH, IC, and IM. An official CFFEX delivery notice is the
event-level source. The provider parses the notice publication/delivery date,
contract identities, and explicit delivery wording. It never derives a
holiday-adjusted date from the third-Friday rule alone. A missing monthly
notice is an explicit incomplete-source failure.

#### Strict 15:35 main-fund-flow ranking

The Eastmoney main-fund-flow page proves the production `clist/get` market
filter, source sort fields, and per-row `f124` update timestamp. The normalized
contract is a 15:35 Asia/Shanghai capture:

- the production client only serves the current trading date;
- local capture must be at or after 15:35:00 and before the date rolls over;
- every row must carry a source `f124` on the requested trading date;
- the upstream order is `f62` descending and normalized ranks are contiguous;
- calls before 15:35, stale dates, missing timestamps, duplicate identities,
  partial pages, or unstable totals fail explicitly.

This is not advertised as a historical 15:35 replay API.

## Core Contracts

### Dragon-tiger discovery request

Add:

```rust
pub struct DragonTigerDiscoveryRequest {
    trading_date: IsoDate,
    exchange: Option<Exchange>,
    limit: PositiveU32,
}

pub trait DragonTigerDiscovery {
    type Error: std::error::Error + Send + Sync + 'static;

    fn discover_dragon_tiger(
        &self,
        request: &DragonTigerDiscoveryRequest,
    ) -> Result<DataBatch<DragonTigerEntry>, Self::Error>;
}
```

The limit is `1..=10_000`. `exchange=None` means Shanghai, Shenzhen and Beijing. The
request never implies current date; callers must supply the trading date explicitly.

Discovery reuses `DragonTigerEntry`. Every record must:

- use the requested trading date;
- match the optional requested exchange;
- have a unique entry ID;
- preserve optional turnover without substituting zero;
- carry Eastmoney evidence whose `source_at` has the requested calendar date.

### Board directory and constituents

Add a `BoardDefinition` sourced record:

```rust
pub struct BoardDefinition {
    board_code: NonEmptyText,
    board_name: NonEmptyText,
    category: BoardCategory,
    member_count: PositiveU32,
    evidence: SourceEvidence,
}
```

Add bounded requests:

```rust
pub struct BoardDirectoryRequest {
    category: BoardCategory,
    limit: PositiveU32,
}

pub struct BoardConstituentRequest {
    board_code: NonEmptyText,
    limit: PositiveU32,
}
```

Add focused traits:

```rust
pub trait BoardDirectoryProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn boards(
        &self,
        request: &BoardDirectoryRequest,
    ) -> Result<DataBatch<BoardDefinition>, Self::Error>;
}

pub trait BoardConstituentProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn board_constituents(
        &self,
        request: &BoardConstituentRequest,
    ) -> Result<DataBatch<BoardMembership>, Self::Error>;
}
```

Both limits are `1..=10_000`. TDX narrows its own constituent limit to the source file's
proved maximum. The existing `BoardMembershipProvider` remains the reverse lookup from
requested instruments to all matching boards.

`BoardDefinition.member_count` counts unique source constituents. `BoardMembership`
continues to contain the normalized instrument, board identity, board name, category and
evidence.

## Provider Architecture

### Eastmoney

`EastmoneyClient` implements `DragonTigerDiscovery`.

The datacenter helper gains an exact-coverage mode that:

1. captures declared page and record totals from page one;
2. keeps a stable page size and unique sort column;
3. requires totals to remain unchanged on every page;
4. rejects missing pages, duplicate source `TRADE_ID` values and count mismatches;
5. reads the complete source day before applying the caller's optional exchange filter and
   result limit.

The complete read prevents a partial first page from being advertised as full-market
discovery. Source error `9201` remains an explicit empty-day result only when the source
envelope proves it.

Mapping validates:

- `SECURITY_CODE` and `SECUCODE` agree;
- `.SH`, `.SZ` and `.BJ` map to the corresponding Core exchange;
- `TRADE_DATE` matches the request;
- `TRADE_ID` is a positive integral source identity;
- amounts are finite and obey existing buy/sell/net invariants;
- turnover is either absent or a non-negative percentage.

### TDX

Add `TdxBoardProvider`, backed by a small injectable `TdxBoardSource` boundary.

The production source wraps `TdxBlockClient`. Deterministic tests inject source
`BlockRecord` collections without opening a socket.

The Provider:

- downloads only the block files needed by the requested operation;
- accepts only industry and concept categories;
- rejects duplicate `(board name, security code)` source rows;
- validates six-digit equity codes and maps `6` to Shanghai and `0`/`3` to Shenzhen;
- rejects unverified code prefixes rather than guessing an exchange;
- generates one observed timestamp and batch ID for one operation;
- never fills `source_at`, because the block protocol does not supply one.

Reverse membership fetches the industry and concept files once each, rejects duplicate
requested instruments and returns every exact source membership for those instruments.
Unknown requested instruments produce no fabricated membership. A fully empty normalized
result is an explicit Provider error so routing can try another source.

## Routing

Add:

```rust
pub type DragonTigerDiscoveryRouter =
    FailoverChain<DragonTigerDiscoveryRequest, DragonTigerEntry>;
pub type BoardDirectoryRouter =
    FailoverChain<BoardDirectoryRequest, BoardDefinition>;
pub type BoardConstituentRouter =
    FailoverChain<BoardConstituentRequest, BoardMembership>;
```

Add source adapters for all three new traits and strengthen the existing board-membership
adapter.

Routing admission verifies:

- output count does not exceed the request limit;
- entry IDs and board/member identities are unique;
- dates and optional exchanges match discovery requests;
- constituent records match the requested board code;
- board directory categories match the request;
- reverse membership records refer only to requested instruments;
- record Provider IDs and batch IDs match the selected source through the existing
  acceptance policy.

Provider parsing failures remain typed source failures and may fail over. No adapter
silently drops an invalid record to manufacture an acceptable batch.

## Capabilities

Add a focused `MarketDiscoveryCapabilities` value with:

```rust
pub struct MarketDiscoveryCapabilities {
    pub dragon_tiger_discovery: bool,
    pub board_directory: bool,
    pub board_memberships: bool,
    pub board_constituents: bool,
}
```

Eastmoney advertises only `dragon_tiger_discovery`. `TdxBoardProvider` advertises the
three board fields. Existing signal capability values retain their current meaning and
wire shape.

## Errors and Bounds

- Dragon-tiger date is mandatory and the limit is at most 10,000.
- Eastmoney uses the existing four-MiB per-response cap and shared one-second gate.
- Datacenter pages use a fixed 500-row size and at most 20 pages.
- TDX block files retain the existing protocol allocation and file-size checks.
- TDX board operations perform at most two remote block-file requests.
- No hidden retry, cookie, device identifier, paid session or cache is introduced.
- Empty success, unstable totals, duplicate identities, unsupported categories and unknown
  source code prefixes are explicit errors.

## Testing

### Deterministic

- Core request bounds and serde round trips.
- `BoardDefinition` validation and sourced-record coverage.
- Eastmoney one-page and multi-page exact-coverage fixtures.
- Eastmoney Shanghai/Shenzhen/Beijing mapping.
- Same-instrument/same-date multi-reason entries remain distinct through `TRADE_ID`.
- Changed totals, missing page data, duplicate IDs, invalid suffixes, wrong dates,
  non-integral IDs and amount inconsistencies fail.
- TDX industry/concept directory, constituents and reverse memberships.
- Duplicate source rows, duplicate requests, unknown boards, invalid categories and
  unverified code prefixes fail.
- Router failover tests for date, exchange, board, uniqueness and limit violations.
- Router admission tests for announcement discovery, global indices, FX,
  economic releases, policy documents, report PDFs, CFFEX delivery events and
  strict post-close rankings.

### Live and load

Eastmoney's live probe adds one recent explicit trading date and prints:

- declared/returned count;
- exchange distribution;
- unique entry IDs;
- batch and record evidence.

TDX gains a dedicated board live probe that prints board counts, one bounded constituent
sample and reverse memberships for one known constituent.

Load probes use concurrency one, at most three attempts and the existing source pacing.
They never scan dates or board names chosen from unbounded user input.

## Documentation and Release

Update crate READMEs, integration docs, capability registry, deployment/operations
documents, compliance probe registry, business rules and release packaging. Document that:

- dragon-tiger discovery is a dated Eastmoney public-data view, not exchange-certified
  data;
- TDX board identities are provider-scoped and have no source timestamp;
- TDX block files contain security codes but no security names. A UI that
  displays TDX board members must join a separately sourced
  `SecurityMetadataProvider` result and keep both evidence records; the board
  Provider never substitutes the code as a fake name.
- TDX index/region blocks and Beijing board membership are not admitted in this slice;
- no cross-source batch is presented as atomic.

## Non-Goals

- Seat discovery for every market entry;
- arbitrary historical date crawling;
- joining boards to dragon-tiger entries inside a Provider;
- claiming TDX board labels are official exchange classifications;
- normalizing the mixed TDX index/region file;
- background polling, persistence or cache;
- global indices or calendars in this slice.
