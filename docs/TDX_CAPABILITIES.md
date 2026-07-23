# TDX capability matrix

`magic-tdx-rs` is the complete pure-Rust TDX source driver in this workspace.
It contains blocking, direct, Tokio async and smart-failover clients plus remote
services and local TDX file readers.

| Area | Implementation and verified boundary |
| --- | --- |
| Quotes | `RealtimeQuotes` on blocking/smart/direct/async; Shanghai, Shenzhen and Beijing |
| K lines | All 12 source categories from 1 minute through yearly; stock and index |
| Five-level books | `OrderBooks` with visible bid/ask depth and record evidence; Shanghai, Shenzhen and Beijing |
| Minute data | Current and dated history plus normalized `MinuteData`; cumulative quantity, no source amount |
| Executed trades | Current and dated history with automatic paging and unknown source sides preserved |
| Security list/metadata | Full count/list and partial normalized metadata for Shanghai/Shenzhen |
| Finance/actions | Realtime 34 fields, market archives, 45 named indicators and XDXR |
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

## Provenance and partial records

TDX Quote and order-book packets contain a raw quote-time area whose format is
still unverified. The adapter leaves `source_at=None` and marks quality
incomplete; it never promotes `observed_at` into source time. These records
cannot enter a downstream five-second freshness gate that requires an
auditable source timestamp.

The security-list packet supplies name and enough evidence to identify ST
names. It does not supply listing date, versioned price-limit rules, board or
source timestamp. Board is visibly derived from exchange/code and the
normalized metadata record remains `Unavailable` with field-level issues.

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

## Real-network acceptance result

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
