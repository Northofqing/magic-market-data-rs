# TDX capability matrix

The first delivery covers the complete pure-Rust capability surface of the
pinned `tdxrs` implementation. The following modules are present in the
`magic-tdx-rs` crate:

| Area | Implementation |
|---|---|
| Blocking pooled client | `TdxHqClient` |
| Direct client | `TdxDirectClient` |
| Tokio async client | `AsyncTdxHqClient` |
| Smart failover client | `TdxSmartClient` |
| Quotes and bars | `protocol::parsers`, `net::*` |
| Five-level order book | `OrderBooks` on blocking, smart and async clients; includes visible bid/ask depth plus record-level source/observation/provider/batch evidence |
| Minute data | current and dated history through `protocol::parsers` and service facades |
| Executed trades | normalized `Trades`/`AsyncTrades` on blocking, smart, direct and async clients; current and dated history with automatic paging, explicit source/observation evidence, and unknown source direction codes preserved |
| Security metadata | normalized `SecurityMetadataProvider` on blocking and smart clients; source name/ST evidence is retained while unavailable listing date, rule version, and source time stay explicit |
| Finance and corporate actions | `protocol::finance_fields`, `protocol::adjuster` |
| Fund data | `fund` |
| Block data | `block` |
| F10/profile | `profile`, `TdxF10Client` |
| Local readers | `reader` |

The core contract also declares `MoneyFlows` and `Auctions`. TDX does not expose
auditable standardized feeds for those families, so their capabilities remain
explicitly `false`; callers receive an unsupported disposition rather than
fabricated zeros or empty successful batches.

Python/PyO3 bindings are excluded. Real-network validation is opt-in through
`examples/live_probe.rs`; deterministic validation is covered by the upstream
unit and parser suite.

The historical financial-file path first uses TDX's official
`data.tdx.com.cn/tdxfin/` distribution endpoint and checks HTTP framing, ZIP
bounds, uncompressed length and CRC before parsing. The `gpcw.txt` byte length
is treated as a bounded allocation hint because the manifest and HTTP object can
be updated independently; a stale length never bypasses the ZIP integrity gate. The quote-server
`0x06B9` report transport remains a fallback because current quote nodes may
return the `gpcw.txt` manifest but an empty fragment for large ZIP files.

On 2026-07-22, the release `live_probe` returned non-empty data for all TDX
families exercised by the example: one stock quote; all 12 stock K-line
categories; five index bars; 27,590 Shanghai security records reported and a
1,000-record list page; 240 current and 240 historical minute points; 20 current
and 20 historical transactions; current finance; 30 corporate-action records;
industry, concept and index blocks; fund quote/bars/actions; and 16 F10
categories. It also downloaded the 5,116,020-byte `gpcw20260331.zip`, validated
and parsed 5,526 market-wide financial records, and extracted all 45 named
indicators for `600396` (华电辽能).

The same probe fetched source names and ST markers for `600396` and `000001`.
The TDX security-list packet does not carry listing dates, versioned price-limit
rules, board fields, or a source timestamp, so board is visibly derived from
exchange/code and the record remains `Unavailable` with field-level quality
issues. Beijing is a first-class core exchange but TDX requests return
`Unsupported` until an official market identifier is verified; Beijing is never
silently mapped to Shenzhen market `0`.

After changing the live sample to Shanghai-listed 华电辽能 (`600396`) and
rerunning on 2026-07-22, the quote price was 14.92, the security-list name was
`华电辽能`, all 12 K-line categories returned, both minute datasets returned
240 rows, trade pagination returned 1,820/1,820 and 2,001/2,001 rows, and the
market-wide archive returned 5,526 records plus 45 named indicators for the
sample security.

The normalized trade probe additionally returned 20/20 current/historical
records with record-level evidence. Paging was exercised across real server
boundaries: current trades returned 1,820/1,820 and historical trades returned
2,001/2,001. TDX direction values `0/1/2` normalize to buy/sell/neutral; observed
post-market values such as `5/8` remain `Unknown(value)` and mark quality
incomplete rather than being guessed.

`ProviderId::LocalTerminal` is reserved for an authorized, read-only local
terminal/SDK adapter. It must never read account, position, cash, or order
state, and it remains unimplemented until the terminal's official local API or
cache format is identified.
