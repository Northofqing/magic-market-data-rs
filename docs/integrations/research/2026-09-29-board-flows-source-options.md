# Eastmoney BoardFlows source options (2026-09-29)

## Finding

No first-party, same-contract replacement for `https://push2.eastmoney.com/api/qt/clist/get`, nor a transport adjustment that resolves an intermittent pre-HTTP-status TLS closure, is established by the evidence below. Eastmoney's [board-flow page](https://data.eastmoney.com/bkzj/) displays industry, concept, and region flows for today, five days, and ten days. That public presentation does not document an interchangeable API host or a stable machine schema. The exact implemented contract is in the [BoardFlows adapter](../../../crates/magic-eastmoney-rs/src/board_flow.rs#L8-L68).

## Contract and failure boundary

The adapter makes one HTTPS `clist/get` request with `pn=1`, `pz<=200`, `po=1`, `np=1`, `fltt=2`, `invt=2`; category filter `m:90+t:{2,3,1}`; and interval sort field `f62`, `f164`, or `f174`. It requires `rc=0`, `data.diff` rows, board code/name, and a positive `f124` Unix timestamp identical across the batch. Empty or inconsistent rows fail. Today's query also requests four size-tier net-flow fields. These are repository requirements, not a published Eastmoney API specification. [Adapter and parser](../../../crates/magic-eastmoney-rs/src/board_flow.rs#L13-L133); [integration contract](../eastmoney-web.md#L80-L87).

The blocking `ureq` transport sets connect/read/write timeouts, disables redirects, bounds response size, enforces HTTPS host admission, and spaces request starts by at least one second per client. `request.call()` precedes status and body handling. A closure before HTTP status therefore supplies no JSON, `f124`, or source evidence to admit. The current BoardFlows path has no retry or host failover. [Transport setup](../../../crates/magic-eastmoney-rs/src/transport.rs#L65-L90); [GET path](../../../crates/magic-eastmoney-rs/src/transport.rs#L153-L169); [host validation](../../../crates/magic-eastmoney-rs/src/transport.rs#L432-L457).

## First-party candidates

| Candidate | What the evidence supports | What it does not support |
| --- | --- | --- |
| [Eastmoney desktop board-flow page](https://data.eastmoney.com/bkzj/) | Public display of the three categories and three periods, labeled with Choice as data source. | Exact API endpoint, required fields, `f124` atomicity, transport reliability. |
| `push2delay.eastmoney.com/api/qt/clist/get` | `push2delay` is an existing registered Eastmoney host. The repo uses a *different* `kline/get` daily instrument-flow path there after prior TLS observations. [Integration notes](../eastmoney-web.md#L124-L130). | Existence or equivalence of `clist/get` on that host; category filters, fields, timestamp, update cadence, or reliability. |
| [Eastmoney H5 board-flow page](https://emdatah5.eastmoney.com/dc/zjlx/block) | Another first-party view with industry/concept/region labels and a narrower visible table. | The complete BoardFlows wire contract. |
| [Eastmoney EmQuant integration](../eastmoney-emquant.md) / Choice | Separate first-party product family. | A credential-free same-contract replacement. No credentials were used. |

## Probe boundary

On 2026-09-29, one credential-free `pz=2` GET to `push2` using the adapter's industry/today query failed at connection establishment in this workspace (`curl`: could not connect to port 443; HTTP code `000`; zero downloaded bytes). The web fetcher also reported the exact `push2` and `push2delay` query URLs inaccessible. No response headers, TLS trace, or JSON body were obtained. This does **not** prove either host is down, closes TLS early, or returns an incompatible schema, and cannot establish a failure rate or host equivalence. [Current query](../../../crates/magic-eastmoney-rs/src/board_flow.rs#L47-L67); [current first-party API URL](https://push2.eastmoney.com/api/qt/clist/get); [candidate first-party API URL](https://push2delay.eastmoney.com/api/qt/clist/get).

## Evidence needed before changing production

Compare the exact bounded query on candidate first-party hosts using the same timeout, redirect, and body limits. Capture status, media type, row count, required fields, and row-wise `f124` across all three categories and admitted intervals, then repeat serially to assess stability. Diagnose where the TLS closure occurs before altering transport behavior. A pre-status closure remains an explicit failure; it cannot be reclassified as a valid response. Any new host or retry policy requires Gate A design and a matching transport registry review. In particular, HTTP 429 and limiter failure must not cause unpaced retries. [BR-009 and BR-010](../../business_rules.md); [transport registry](../http-transports.tsv); [engineering gates](../../ENGINEERING_RULES.md).
