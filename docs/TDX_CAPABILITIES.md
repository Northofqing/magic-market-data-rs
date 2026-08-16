# TDX capability matrix

`magic-tdx-rs` is the complete pure-Rust TDX source driver in this workspace.
It contains blocking, direct, Tokio async and smart-failover clients plus remote
services and local TDX file readers.

| Area | Implementation and verified boundary |
| --- | --- |
| Quotes | `RealtimeQuotes` on blocking/smart/direct/async; Shanghai, Shenzhen and Beijing |
| K lines | All 12 source categories from 1 minute through yearly; stock and index; normalized latest-N requests page atomically across the 800-row wire limit for the full positive `u16` request domain |
| Five-level books | `OrderBooks` with visible bid/ask depth and record evidence; Shanghai, Shenzhen and Beijing |
| Minute data | Current and dated history plus normalized `MinuteData`; cumulative quantity, no source amount |
| Executed trades | Current and dated history with automatic paging and unknown source sides preserved |
| Security list/metadata | Full count/list plus finance-backed listing dates in the normalized Gateway for Shanghai/Shenzhen; rule and board authority remain partial |
| Finance/actions | Realtime 34 fields, market archives, 45 named indicators, raw XDXR and normalized `CorporateActions` for Shanghai/Shenzhen |
| Funds | Quotes, bars, finance and XDXR |
| Blocks | Industry, concept and index classifications |
| F10/profile | Categories, named sections and complete payloads |
| Local readers | Daily/minute bars, finance and block files |
| Money flow | `false`; field-specific `Unsupported` |
| Call auction | `false`; field-specific `Unsupported` |

## Beijing market evidence

Live protocol validation on 2026-07-23 tried TDX markets `0`, `1` and `2` for
太湖远大 `920118`. Markets `0` and `1` returned a mismatched Shanghai record;
only market `2` returned `(market=2, code=920118, price=16.91)`. The adapter
therefore maps `Exchange::Beijing` exclusively to `2` and still rejects any
response whose market/code does not match the request.

The release probe then returned:

- one normalized Beijing Quote at 16.91;
- five Beijing daily bars;
- five bid and five ask levels;
- 120 current-session and 240 previous-session minute points;
- 20 current trades.

The normalized minute batch was complete and carried source time through
`2026-07-23T11:30:00+08:00`. Quantity is the cumulative source quantity;
TDX minute packets do not expose an auditable cumulative amount, so that field
remains `None`.

Beijing security count returned 364, but live-verified servers close the
`market=2` security-list request. Because the list packet is required for name
metadata, a Beijing `SecurityMetadataProvider` request returns an immediate
`Unsupported` explaining this endpoint boundary. It is not retried as a fake
Shanghai/Shenzhen request. Shanghai/Shenzhen metadata remains available.

The admitted normalized `TdxSecurityProfileProvider` combines that exact
Shanghai/Shenzhen identity and optional finance-backed listing date with the
unique public F10 `公司概况` section. It preserves up to 256 ordered non-empty
source lines as facts and deliberately leaves unproved industry and share-count
fields unavailable. The exact 1..=8 equity scope and 2026-08-14 live/load
evidence are recorded in
[`integrations/tdx-public-security-profile.md`](integrations/tdx-public-security-profile.md).

## Provenance and partial records

TDX Quote and order-book packets contain a raw quote-time area whose format is
still unverified. The adapter leaves `source_at=None` and marks quality
incomplete; it never promotes `observed_at` into source time. These records
cannot enter a downstream five-second freshness gate that requires an
auditable source timestamp.

That distinction is preserved by the external derived products. On 2026-08-16
`OutcomeDailyBars` passed two live and three serial requests for 600396.SH with
20 exact daily bars ending on the requested 2026-08-14 `through` date, so that
TDX-only product is admitted. `T0Evidence` also completed two live and three
serial four-family captures (Quote, five-level book, 20 daily bars and 20
five-minute bars), but Quote/book retained `source_at=None` and
`status=Unavailable`. It is therefore exposed only as an explicit opt-in gRPC
diagnostic with `admission=UNADMITTED` and `complete=false`; successful TCP
reads do not promote it.

The security-list packet supplies name and enough evidence to identify ST
names. The normalized Gateway joins it with the exact requested finance record
to supply a validated listing date; mismatched identities, malformed dates and
future dates fail explicitly. TDX still does not supply versioned price-limit
rules, authoritative board identity or source timestamp. Board is visibly
derived from exchange/code and the normalized metadata record remains
`Unavailable` with field-level issues for those fields.

The raw XDXR parser validates complete response framing, the returned
market/code identity, Gregorian dates and finite source terms across the entire
response. The normalized `CorporateActions` Gateway validates the whole
response before applying the requested inclusive date range, so an invalid or
unknown row outside the range cannot be hidden by projection. The contract maps
all 14 protocol categories without inventing unverified units. Categories 2
through 10 preserve the complete before/after tradable/total tuple; categories
13 and 14 preserve exercise price and source quantity. Category 11 preserves
the provider's broader “capital rescaling” classification, rather than
narrowing it to a split. The physical units of those quantities and the
category 11/12 `suogu` field are not independently proved by the upstream
decoder, so they carry `UnverifiedSourceUnit::ProviderNative` and cannot be
interpreted as shares, lots or adjustment ratios. All records carry
`InstrumentId`, `SourceEvidence` and request-bound `DataBatch` provenance. A
complete historical response with no records in the requested range is
represented as a verified empty batch.
The response also carries an explicit China `admission_as_of`; future request
coverage or effective dates fail before they can authorize a discontinuity.
TDX does not provide an auditable XDXR source timestamp, so `source_at` remains
`None`. Beijing requests return `Unsupported` before transport rather than
being remapped to another market.

TDX current trade direction values `0/1/2` map to buy/sell/neutral. Other
observed values, including post-market values such as `5` and `8`, remain
`Unknown(value)` and make quality incomplete rather than being guessed.

## Explicit unsupported families

The normalized `MoneyFlow` contract requires auditable main/net inflow fields
and source methodology. TDX Quote/trade packets do not provide them. The
normalized `AuctionSnapshot` requires indicative price and matched/unmatched
quantities; the implemented packets do not provide those fields. Both traits
are callable but capabilities remain `false`, and both return field-specific
`Unsupported` errors instead of zeros or empty successful batches.

## Real-network acceptance results

On 2026-07-27 the lifecycle raw-inventory command exited zero:

```bash
TDX_LIFECYCLE_RAW_ONLY=1 \
  cargo run -p magic-tdx-rs --example live_probe --locked --offline
```

Using `杭州联通J2 (60.12.136.250:7709)`, it returned 45 `600519` rows with
category histogram `{1: 30, 2: 7, 3: 1, 5: 5, 9: 1, 14: 1}`. A subsequent full
probe validated listing dates for 华电辽能 `600396` (`2001-03-28`), 平安银行
`000001` (`1991-04-03`) and 贵州茅台 `600519` (`2001-08-27`). Its lifecycle
section validated every raw row before returning the exact two complete 2024
distribution records and a complete request-bound empty 1900 result. The full
multipurpose process later exited one for unrelated five-minute timestamp and
block-index transport checks; it is not recorded as a whole-probe pass. Exact
commands, terms and boundaries are recorded in
[the 2026-07-27 TDX lifecycle evidence](evidence/2026-07-27-tdx-lifecycle.md).

On 2026-07-23 this command exited zero with
`live_probe_status=passed`:

```bash
cargo run -p magic-tdx-rs --example live_probe --release
```

In addition to the Beijing evidence above, it returned all 12 stock K-line
categories, five index bars, Shanghai/Shenzhen metadata for 华电辽能 `600396`
and 平安银行 `000001`, 20 current and 20 historical trades, cross-page
1,820/1,820 current and 2,001/2,001 historical trades, realtime finance, 30
XDXR records, three block families, fund data and 16 F10 categories. It
downloaded and validated the `gpcw20260331.zip` archive, parsed 5,526 records
and extracted all 45 named indicators.

The financial-file path uses TDX's `data.tdx.com.cn/tdxfin/` distribution
endpoint and validates HTTP framing, ZIP bounds, uncompressed length and CRC.
The manifest byte length is only a bounded allocation hint because the manifest
and object can change independently. The quote-server report transport remains
a fallback because current nodes can return the manifest but an empty large
file fragment.

Python/PyO3 bindings are excluded. `ProviderId::LocalTerminal` remains reserved
for a separately authorized read-only terminal adapter and must never expose
account, position, cash or order state.

The optional local-terminal monitor uses direct safe Rust polling, not a Python
binding or vendor DLL. Its selected data path is the vendor-documented TQ-Local endpoint at
exactly `http://127.0.0.1:17709/`: bounded, single-flight read-only HTTP
polling with no proxy or redirect. The installed CPython-dependent callback path
is excluded. Until exact response
schema, compatibility and bounded live evidence pass, every LocalTerminal
input and LocalAnalysis anomaly capability remains false and no optional
listener may start. The source must not send any account/trading method.
