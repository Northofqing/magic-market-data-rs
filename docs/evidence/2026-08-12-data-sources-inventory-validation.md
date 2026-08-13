# Data sources inventory validation — 2026-08-12

Status: provider-probe execution completed across 2026-08-12 and the bounded
2026-08-13 transient-source reruns; application E2E is blocked by repository
scope. This is a fail-closed report for every data-acquisition row in
`docs/data-sources-inventory.md`. It does not convert a successful lower-level
Provider probe into a successful downstream gateway, scheduler, router, search,
or analysis result.

## Scope and evidence rules

The inventory describes an application tree containing
`src/data_gateway/**`, `src/search_service/**`, `src/news/**`,
`src/calendar.rs`, and `src/bin/tdx_server_probe.rs`. This repository has no
root `src/` tree and intentionally supplies upstream Provider crates rather
than the downstream `stock_analysis::data_gateway` application. The inventory
also references BR-085 through BR-231, while this repository's registered
rules currently end at BR-044.

Consequently:

- all 36 application E2E paths are `blocked_missing_application_source`;
- every executable, non-duplicated lower-level probe was run where its source
  and session permitted;
- a request that returned data but remains unadmitted is reported as
  `diagnostic_unadmitted`, never `passed`;
- explicit transport, schema, source-ordering, admission, session, and missing
  implementation failures are preserved;
- public TDX TCP probes do not stand in for the separate official TQ-Local
  loopback monitor.

On Windows, many crates use the example name `live_probe`. Every accepted
result below came from a crate-specific target directory. Earlier runs against
a shared target were discarded because concurrent Cargo processes can select
the wrong same-named executable.

## Row-by-row result

| Line | Inventory capability | Lower-level result through 2026-08-13 | Application E2E |
|---:|---|---|---|
| 14 | A-share real-time quote route | `partial_pass`: TDX, Tencent, and Sina Provider quote probes passed independently. A continuous-session router rerun passed and selected Tencent after rejecting incomplete TDX evidence. The executable still implements only TDX→Tencent quote fallback, not the documented TDX→Tencent→Sina multi-family route. | `blocked_missing_application_source` |
| 15 | Five-level order book | `partial_pass`: TDX, Tencent, and Sina Provider books passed; no three-provider order-book router exists here. | `blocked_missing_application_source` |
| 16 | Minute bars | `partial_pass`: TDX, Tencent, and Sina minute probes passed; application minute routing and derived average-price contracts are absent. | `blocked_missing_application_source` |
| 17 | Five-minute bars | `partial_pass`: TDX normalized five-minute bars passed; application cache and 30-second T0 scheduling are absent. | `blocked_missing_application_source` |
| 18 | Daily bars | `partial_pass`: TDX, Tencent, and Sina passed. Baidu returned one complete record but was explicitly `diagnostic_complete_unadmitted`; the four-provider application router is absent. | `blocked_missing_application_source` |
| 19 | Outcome daily bars V2 | `not_mapped`: `OutcomeDailyBarsV2` has no implementation or executable in this tree. | `blocked_missing_application_source` |
| 20 | T0 evidence chain | `not_mapped`: Provider primitives passed, but batch reuse, content digest, cache, and scheduler evidence chain do not exist here. | `blocked_missing_application_source` |
| 26 | Four-feed global news aggregate | `failed_partial`: CLS passed. Jin10 flash returned five complete items, but its calendar stage failed. Eastmoney failed strict host provenance for a current `stock.eastmoney.com` article. ThePaper first passed a transient rerun and then failed its current native-link contract. No aggregate success is claimed. | `blocked_missing_application_source` |
| 27 | Macro news search | `not_mapped`: individual feed probes exist, but concurrent gateway/search/LLM composition is absent. | `blocked_missing_application_source` |
| 28 | Instrument news | `passed_after_transient_rerun`: the current Sina rerun returned three ordered records for each of `600396` and `000001`. The earlier newest-first rejection remains dated source-drift evidence. | `blocked_missing_application_source` |
| 29 | CNInfo market announcements | `provider_pass`: admitted batches contained 1 instrument announcement, 300 market announcements, and 1 investor question. | `blocked_missing_application_source` |
| 30 | Market news event pipeline | `not_mapped`: simhash, event conversion, industry-chain, and stock-selection pipeline are absent. | `blocked_missing_application_source` |
| 36 | Board capital flow | `failed`: Eastmoney industry, concept, and region requests each failed TLS because the peer closed without `close_notify`. | `blocked_missing_application_source` |
| 37 | Board constituents | `provider_pass`: TDX board directory→exact concept constituents→reverse membership passed; 400 members were returned for `人工智能`. The Provider request limit is 1,000 rather than the inventory's 10,000. | `blocked_missing_application_source` |
| 38 | Board directory | `partial_pass`: the tested TDX concept directory returned 269 entries. The parser supports concept and industry, not the inventory's region directory. | `blocked_missing_application_source` |
| 39 | Concept-board net-inflow ranking | `not_mapped`: the exact concept-board clist endpoint is absent. Eastmoney board-flow and security Top-N are different contracts. | `blocked_missing_application_source` |
| 45 | TDX limit-up/streak pool | `not_mapped_as_declared`: the TDX crate has no limit-pool implementation. The alternative Eastmoney source admitted Upper, Broken, and Previous pools; Lower was a source-backed typed `VerifiedEmpty`, not an ordinary empty success. | `blocked_missing_application_source` |
| 46 | Security list | `partial_pass`: Tencent and Sina metadata requests ran. Sina correctly marked incomplete lifecycle/price-limit evidence unavailable; no Tencent→Sina application assembly exists. | `blocked_missing_application_source` |
| 47 | Trading calendar | `not_mapped`: the documented local calendar and authority-URL validation implementation is absent. | `blocked_missing_application_source` |
| 53 | Instrument capital flow | `diagnostic_unadmitted`: Eastmoney minute and daily requests each returned five records, while capability admission remained false. | `blocked_missing_application_source` |
| 54 | HKEX northbound daily | `provider_pass`: one SSE-channel and one SZSE-channel record for 2026-08-11 passed with complete quality. | `blocked_missing_application_source` |
| 55 | Sell-side consensus | `not_mapped_as_declared`: Eastmoney reports `consensus=false`; only target-price consensus exists. The rerun produced a source-backed verified empty target-price result for `600396`; net-profit/revenue/ROE consensus is still not proved. | `blocked_missing_application_source` |
| 56 | Research reports | `partial_pass`: Eastmoney instrument and industry research batches were admitted; the target-price rerun returned an admitted, source-backed verified empty result. Application filtering and invocation are absent. | `blocked_missing_application_source` |
| 57 | Three financial statements | `provider_pass`: Sina balance sheet, income statement, and cash-flow probe stages passed. | `blocked_missing_application_source` |
| 58 | Market statistics | `provider_pass`: Tencent equity/index/ETF market-statistics samples passed. | `blocked_missing_application_source` |
| 59 | Dragon-tiger list | `provider_pass`: Eastmoney entry, seat, and all-market batches were admitted. | `blocked_missing_application_source` |
| 60 | Block trades | `provider_pass`: Eastmoney block-trade batch was admitted. | `blocked_missing_application_source` |
| 61 | Provider Top-N | `composition_pass`: after the close, VolumeRatio and MainNetInflow each admitted 20 of 5,551 candidates with zero failures. | `blocked_missing_application_source` |
| 67 | Six A-share indices | `not_mapped`: Tencent supports index instruments, but the executable quote sample constructs equities and does not prove the exact six-index request. | `blocked_missing_application_source` |
| 68 | Three US indices | `provider_pass`: the Sina global-index batch returned six indices including Dow, Nasdaq, and S&P 500. | `blocked_missing_application_source` |
| 69 | USD/CNY | `provider_pass`: the Sina FX batch returned eight pairs including USD/CNY. | `blocked_missing_application_source` |
| 75 | CFFEX delivery calendar | `failed_unadmitted`: Rustls connection initialization ended with an unexpected EOF before a diagnostic success marker; admission remains false. | `blocked_missing_application_source` |
| 81 | Economic calendar | `failed`: Jin10 flash succeeded, but the requested calendar stage returned no eligible public economic releases. | `blocked_missing_application_source` |
| 87 | General web research | `not_mapped`: Bocha, Tavily, and SerpAPI application code and credential mapping are absent from this workspace. | `blocked_missing_application_source` |
| 93 | Listing date/corporate actions | `provider_pass`: TDX verified the 600519 listing date and source-backed corporate-action samples, including a verified historical empty case. | `blocked_missing_application_source` |
| 94 | Position board membership | `partial_pass`: TDX reverse membership returned 24 boards for the probe instrument. The application position input and primary-board selection are absent. | `blocked_missing_application_source` |

## Provider execution summary

| Probe | Result | Fresh evidence |
|---|---|---|
| Magic TDX public TCP full probe | `passed` | Connected to `60.12.136.250:7709`; quotes, books, all 12 bar categories, minutes, current/history transactions, metadata, lifecycle/XDXR, finance archives, funds, boards, F10, and pagination passed. Raw trade-side values 5 and 8 were retained as `Unknown(raw)` rather than dropped. Latest archive integrity decoded 259 records; the fixed evidenced 2026-03-31 archive decoded 5,540 and mapped 45 indicators for 600396. |
| Magic TDX board probe | `passed` | 269 concept directory rows, 400 `人工智能` constituents, and 24 reverse memberships. |
| Tencent | `passed` | SH/SZ/BJ quote/book/metadata, bar intervals, minute/trade, and market-statistics stages passed. |
| Sina full market probe | `passed` | 6 global indices, 8 FX pairs, three quotes/books/metadata sets, supported bars, Beijing samples, and option samples. Incomplete metadata remained explicitly unavailable. |
| Sina instrument news | `passed_after_transient_rerun` | The current rerun returned three ordered records for both requested symbols; the earlier newest-first rejection remains recorded. |
| Baidu bars | `diagnostic_unadmitted` | One complete daily record; capability gate remained false. |
| CLS | `admitted` | One complete fresh global-news record. |
| Jin10 | `failed_partial` | Five flash records succeeded; economic calendar returned no eligible releases. |
| ThePaper | `failed_after_source_drift` | After one transient rerun returned five complete records, the latest bounded rerun rejected a native row carrying an external link. No origin policy was relaxed. |
| CNInfo | `admitted` | 1 instrument announcement, 300 market announcements, 1 investor question. |
| Exchange/HKEX | `passed` | SSE/SZSE official datasets and two HKEX northbound channels passed. |
| CFFEX | `failed_unadmitted` | TLS unexpected EOF before diagnostic completion. |
| Eastmoney full suite | `failed_partial` | Most research/capital/limit-pool/ranking families admitted; all three board-flow requests failed TLS and global news failed its approved-host contract. Fund flow remained diagnostic-only. |
| Eastmoney target price | `admitted_verified_empty` | The rerun reached `reportapi.eastmoney.com` and returned a source-backed exact empty result for the bounded `600396` date range. |
| Provider Top-N composition | `admitted` | Two ranking families, 20 returned per family from 5,551 candidates. |
| Market router | `passed_limited_contract` | Continuous-session rerun selected Tencent after TDX evidence rejection and passed the five-second quote gate. It still proves only quote TDX→Tencent, not the inventory's complete route. |

## 2026-08-13 incomplete-item rerun

The previously incomplete, transient-failure and session-dependent probes were
rerun during the Shanghai continuous session. These results supersede only the
corresponding dated status above; they do not create the missing downstream
application code or promote any admission:

| Item | Rerun result | Exact evidence |
|---|---|---|
| Quote router | `passed_limited_contract` | `600396.SH` was requested during continuous trading. The TDX public-TCP candidate was rejected for incomplete quote quality and Tencent was selected with price `17.38`, source time `2026-08-13T11:16:30+08:00`, age `3696 ms`, and `router_live_probe_status=passed`. The executable still proves only quote TDX→Tencent, not the documented third Sina leg or the other market-data families. |
| Sina instrument news | `passed_after_transient_rerun` | Both `600396` and `000001` returned three ordered records and `instrument_news_live_probe_status=passed`. The earlier source-ordering rejection remains dated transient evidence. |
| Eastmoney target-price consensus | `admitted_verified_empty` | The exact `600396` request for `2026-01-01..2026-07-27` returned source-backed `hits=0`, `size=0`, `TotalPage=0`, `data=[]`; this is a verified empty result, not fabricated consensus. Net-profit/revenue/ROE consensus remains unsupported. |
| Jin10 calendar | `failed_partial` | Five flash records remained complete, but the calendar stage again returned `Jin10 returned no eligible public economic releases`; process exit was 1. |
| CFFEX delivery calendar, Rustls | `failed_unadmitted` | `https://www.cffex.com.cn/cn/jystz.html` ended TLS initialization with unexpected EOF. |
| CFFEX delivery calendar, Native TLS | `failed_unadmitted` | The feature-enabled second backend also failed, with Windows `native_tls` reporting no credentials in the security package. No diagnostic success/admission marker was emitted. |
| Eastmoney concept board flow | `failed_unadmitted` | The bounded `board-flow-concept` request failed because `push2.eastmoney.com` closed TLS without `close_notify`. |
| Eastmoney global news | `failed_contract` | A current article used `stock.eastmoney.com`, which is not an admitted global-news host; the strict provenance policy rejected it. |
| ThePaper global news | `failed_contract` | The current native row carried an external link and was rejected by the native-source contract. This demonstrates source drift after the earlier successful transient rerun. |

The last five failures cannot be converted to success by weakening TLS, host,
origin or schema rules. TDX local data is not semantically equivalent to news,
economic releases or a futures-exchange delivery calendar.

## TDX local read-only query and fallback diagnostics

In addition to the admitted-false Rust monitor, the fixed official loopback was
queried directly and sequentially to determine which failed inventory families
could plausibly have a TDX-local fallback. These calls were diagnostic only:
the Rust diagnostic allowlist remains exactly `get_stock_list`,
`get_pricevol`, and `get_market_snapshot`; no data-family admission was
widened.

| TQ-Local method/family | Result | Fallback judgement |
|---|---|---|
| `get_pricevol`, two equities | `passed_sustained` | Used by the Rust monitor; eligible diagnostic source for price and cumulative volume only. |
| `get_market_snapshot` | `passed_sustained` | Returned exact price, cumulative lots and amount-in-ten-thousand-CNY converted with checked decimal arithmetic. Arrays shaped as five levels had only level one non-zero for both tested equities, so this is not evidence for a complete five-level book fallback. |
| Six A-share indices | `passed_diagnostic` | One `get_pricevol` request returned all six: `000001.SH=3963.15`, `399001.SZ=14519.97`, `399006.SZ=3660.25`, `000688.SH=1770.03`, `000300.SH=4714.06`, `000905.SH=8113.14`. This proves a candidate TDX-local fallback but does not satisfy the inventory's existing Tencent-only identity/5-second contract without a Gate A contract change. |
| Daily bars | `passed_diagnostic` | A five-record `1d` request returned OHLC, volume and amount. It is a plausible daily-bar fallback after a normalized Rust contract is implemented. |
| One- and five-minute bars | `unavailable` | Both explicit current-session range requests returned HTTP/RPC success but `Value=[]` (`KlineTotal=1`), so local TDX did not provide usable minute records in this installation/session. |
| Tick through `get_market_data` | `unsupported_or_empty` | The installed wrapper's valid period list excludes `tick`; the diagnostic returned no records. No tick fallback is claimed. |
| Trading calendar/dates | `passed_diagnostic_unverified` | Returned local SH dates for August 2026. It remains a local downloaded-data view and cannot replace the inventory's exchange-authority validation until cross-checked. |
| A-share stock list | `passed_diagnostic` | Returned 5,552 exact symbols spanning SH/SZ/BJ. |
| Board directory | `passed_diagnostic` | Returned industry, concept and region rows, including `人工智能=880948.SH` and regional boards. This closes the earlier query-level region-directory gap, not the missing application gateway. |
| Board constituents | `passed_diagnostic` | `880948.SH` returned a large bounded exact symbol list. Candidate fallback for query-level membership only. |
| Reverse board membership | `passed_diagnostic` | `600396.SH` returned 19 memberships including industry `电力`, region `辽宁板块` and several concepts. Primary-board selection remains downstream policy. |
| Security search/basic/extended info | `passed_diagnostic` | Search mapped `华电辽能` to `600396.SH`; basic and extended calls returned name, industry, region, listing date, capital, financial and current-derived fields. Field freshness/provenance is not yet normalized. |
| Corporate actions/dividend factors | `passed_diagnostic` | `600396.SH` returned 15 dated factor rows. This is a plausible lifecycle fallback only after exact factor semantics are registered. |
| Share capital | `passed_diagnostic` | Two requested dates returned exact total and float capital for `600396.SH`. |
| IPO subscription list | `passed_diagnostic` | Returned three future subscription rows. Zero issue price/max-subscription fields remain source values, not inferred data. |
| Index-to-ETF relation | `passed_diagnostic` | `000300.SH` returned 30 tracking ETFs with IOPV/current/previous-close and size fields. |
| Professional financial data | `unavailable` | The RPC succeeded but returned only null `announce_time`/`tag_time`; the installation does not currently have usable downloaded professional financial tables for the tested symbol. |
| Per-stock transaction table | `unavailable` | RPC success returned the symbol mapped to null for tested `GP1..GP5`. |
| Market transaction table | `unavailable` | RPC success returned no value for tested `SC1..SC5`. |
| Single-stock professional fields | `unavailable` | Tested `GO1..GO4,GO47` all returned source zero. They are not accepted as proof of real measurements. |
| Board transaction fields | `partial_unmapped` | `880948.SH` returned an actual `BK9` row for `20260813`, but the field's exact unit/meaning is not registered and cannot stand in for Eastmoney main-net-inflow ranking. |
| Daily statistics (`get_exday_data`) | `unavailable` | Returned one dated row whose order/cancel/amount/volume fields were all source zero. |
| Limit-up/down (`get_zdt_data`) | `invalid_source_shape` | RPC reported success, but multiple requests returned a contradictory `Code=".SZ"` and all-zero values even for Shanghai symbols. It is rejected as a limit-pool fallback. |
| Convertible-bond sample | `verified_empty_or_stale_sample` | The installed sample symbol returned an empty list; no capability claim is made. |

The TQ wrapper also exposes callback subscription functions, but
`subscribe_quote` is documented by the installed vendor wrapper as having no
actual function, while `subscribe_hq` registers a CPython callback through
`Register_DataTransferFunc`. That path contradicts the selected zero-Python
safe-Rust design and was not invoked. The supported Rust listening model is the
bounded fixed-loopback polling service, not a disguised or unverified native
callback.

### Sustained local-listener evidence

A new 60-cycle Windows run used two explicit equities (`600396.SH` and
`000001.SZ`). It exited zero and produced 190 fully decoded 4-byte-big-endian
frames: 120 fast observations, 59 snapshot observations, six analysis updates,
one discovery candidate, one loopback health event, one running event, one
typed diagnostic completion and one joined snapshot-worker shutdown. There
were 91 instrument-bearing frames for `600396` and 89 for `000001`.

The final `600396.SH` snapshot carried price `17.51`, cumulative volume
`1720511` lots and cumulative amount `2993574700` CNY; price and volume both
matched the latest fast sample. All price/volume/amount analysis families
reached `warmed_up`, but every emitted admission remained false. The capture is
the ignored local artifact `target/tdx-e2e-60.frames` and is not a production
promotion marker.

### 2026-08-13 live anomaly lifecycle run

A later bounded run crossed the 13:00 Asia/Shanghai afternoon open and stopped
normally after all 6,000 configured scheduler cycles. All 18,287 service
events were decoded exactly from 9,990,734 bytes of four-byte-big-endian framed
JSON. The capture contained 11,996 fast observations, 5,994 snapshot
observations, 274 analysis transitions, three discovery generations and two
typed transient TQ timeouts followed by successful rediscovery and generation
revalidation. The snapshot worker joined on shutdown.

The run deliberately used diagnostic trigger values of one lot and one CNY to
exercise lifecycle transitions; those values are not defaults or production
recommendations. For `600396.SH`, cumulative volume advanced from `1720511` to
`1732128` lots and cumulative amount from `2993574700` to `3013943800` CNY.
The volume monitor emitted `triggered(value=11617)`,
`entered_cooling_down`, and `rearmed(value=0)`. The amount monitor emitted the
same lifecycle with `triggered(value=20369100)`. `000001.SZ` did not inherit
either trigger, proving per-instrument isolation in the real service path. All
events remained `admitted=false`.

A later 3,000-cycle, 16-equity live run superseded the earlier static-price
captures. It ran for 1,389.5 seconds and exited zero without external
termination. Its 30,757,185-byte output decoded as 48,212 complete four-byte
big-endian frames: 47,989 fast observations, 29 snapshot observations and 174
analysis updates, followed by `diagnostic_completed` and a joined
`snapshot_worker_stopped`. Two real loopback timeouts produced explicit reset
and waiting events; each was followed by rediscovery, A-share-universe
revalidation and a new generation, for three generations in total.

The longer run captured actual vendor batch refreshes. Price completed 11
`triggered -> entered_cooling_down -> rearmed` lifecycles and cumulative volume
completed 15. Price examples include `600050.SH` from `4.33` to `4.35`,
`600396.SH` from `17.51` to `17.76`, and `600519.SH` from `1354.82` to
`1358.01`. All 16 instruments warmed independently in every generation; only
instruments satisfying the injected rule emitted transitions. This supplies
real price/volume trigger, cooldown, rearm, per-instrument isolation and
timeout-recovery evidence. The diagnostic thresholds were deliberately tiny to
exercise state transitions and are not production recommendations. All events
remained `admitted=false`.

The earlier static captures remain useful source-cadence evidence. An explicit
unadmitted `refresh_cache(market=AG, force=false)` diagnostic returned success
in 21.5 ms but did not change any of four immediate before/after samples. The
installed wrapper rejects `period=tick` through its own period allowlist,
`subscribe_quote` says it has no actual function, and `subscribe_hq` requires
the excluded CPython/GIL callback path. TQ-Local therefore exposes interval
batch refreshes to this Rust poller, not a source-timestamped tick stream; no
tick-latency or source-freshness claim is made.

A post-open `get_market_data(period=1m,count=5)` diagnostic also returned
HTTP/RPC success but an empty `Value` for both `600396.SH` (about 12.9 seconds)
and `300059.SZ` (about 50 ms). Thus neither local minute data nor the ineffective
immediate cache-refresh call is an independent tick-stream fallback in this
installation; the selected listener continues to use the observed
`get_pricevol` batch refreshes.

Live execution also exposed two monitor-service contract bugs that were fixed
and regression-tested: continuous polling through the lunch break now treats
`Break -> Afternoon` as an explicit `midday_break` reset, and rule-level reset
frames now preserve an exact reason such as `sampling_gap` rather than a bare
`reset` marker.

## Correctness fixes discovered by live execution

The provider probes exposed three local correctness defects, which were fixed
without weakening source or admission contracts:

1. TDX current/history transaction parsers no longer reject raw side values
   above 2. The normalized open enum preserves 5/8 as `Unknown(raw)`, marks
   side availability unavailable, and retains the record.
2. TDX financial indicator extraction now rejects short field vectors instead
   of silently replacing missing fields with `0.0`. The live probe separates
   latest-archive structural integrity from fixed, source-backed symbol-field
   mapping evidence.
3. Eastmoney limit-pool `tc=0` is represented as typed `VerifiedEmpty`, and
   minute-resolution news source times are normalized to explicit
   Asia/Shanghai seconds before entering the strict Core evidence contract.

Sina raw-text parsing was hardened so JavaScript `<` comparisons and nested
non-anchor elements are not interpreted as article tags. Its live newest-first
failure remains explicit; records were not reordered to force a pass.

## Local TDX terminal status (separate from the inventory)

The selected path is the official TQ-Local HTTP service at fixed origin
`http://127.0.0.1:17709/`, not a native listener. Rust performs no Python import
or vendor-DLL load/call.

On 2026-08-13, `magic-tdx-native-bridge --discover` found one current-user,
current-session `TdxW.exe` and returned typed `discovered`/exit 0. It recorded
PID 31472, the process creation identity, x86-64 architecture and executable
SHA-256 `58bd2117ec86e8c063639f7adae4218011bb93998e3d93dcd286672d1978736b`.
That helper performs discovery only.

The safe Rust loopback client then completed `get_pricevol` calls. Ten direct
diagnostic calls measured approximately 19--51 ms; Rust observations measured
approximately 12--20 ms after warm-up. A full monitor-service run automatically
progressed through discovery candidate, loopback schema health and running, and
emitted price plus cumulative volume in lots while amount and source-record
count remained typed unavailable on that fast method.

The thirteenth bounded read used `get_market_snapshot` for exact request
instrument `600396.SH` with minimal field list
`[Amount, Now, Volume, LastClose]`. The captured HTTP 200 result contained
`Amount="127354.65"`, `Now="17.62"`, `Volume="735536"` and
`LastClose="17.18"`, plus bounded ignored vendor fields. The installed vendor
sample defines total amount in ten-thousand CNY, and official TQ policy defines
market volume in lots. Checked string-decimal conversion therefore produced
exactly `1273546500` CNY without floating point or rounding and retained
`735536` lots. The complete captured result is checked in as the parser fixture.
It contained no source timestamp or source record count; `ItemNum` was not
promoted to either fact. An earlier cold snapshot completed in about 11 seconds,
while this capture completed in under one second; neither is a production
cadence.

The current server requires an explicitly typed equity watchlist such as
`EQUITY:SH:600396` and all 38 monitoring/resource/output switch-value pairs,
while TDX path, helper path and the fixed endpoint are not configurable.
Discovery events retain numeric file/product version, version source or a
structured version-read failure, and an explicit cadence rechecks process
identity. Deterministic runtime
tests prove that a separately paced capacity-one snapshot worker cannot block
or replay the fast families and that its failure resets only amount. Server
stdout uses bounded four-byte big-endian length-prefixed JSON frames, not JSON
Lines, and there is no inbound listener. A bounded non-blocking stdout queue
uses only the fail-closed `stop` slow-consumer policy; queue pressure is not
silently dropped or allowed to block polling. Windows packaging installs the server
and discovery helper together as diagnostic/admitted=false artifacts; this
packaging change is not live-admission evidence.

A subsequent bounded Windows end-to-end diagnostic completed 12 configured
scheduler cycles and exited zero. The discovery event retained PID 31472,
numeric version `1.0.0.1` and SHA-256
`58bd2117ec86e8c063639f7adae4218011bb93998e3d93dcd286672d1978736b`.
The final fast observation carried price `17.18` CNY/share and cumulative volume
`1447695` lots. The snapshot path produced cumulative amount `2520326100` CNY;
its price and volume both matched the latest fast sample. Price, volume and
amount monitors each reached `warmed_up`; none was labelled admitted. Shutdown
reported the snapshot worker joined. The 4-byte big-endian framed capture was
written to ignored local artifact `target/tdx-monitor-e2e.frames`; it is runtime
evidence, not a checked-in output or a capability-promotion marker.

All LocalTerminal/LocalAnalysis production admission constants remain false
until field/session reset semantics, calendar integration and sustained shadow
evidence close. This is a data-family admission condition, not an unresolved
Python or native-ABI condition.

## Final verdict

The executable Provider surface corresponding to the inventory was exercised
to completion across 2026-08-12 and the bounded 2026-08-13 transient-source
reruns, with explicit passes, partial failures,
diagnostic-only results, session blocks, and missing mappings above. The claim
“all inventory data acquisition passed” would be false:

- all 36 downstream application E2E paths are outside this repository;
- 10 rows have no equivalent executable mapping here;
- several mapped sources failed live transport/schema/source-ordering checks;
- several successful requests remain deliberately unadmitted; and
- the local TDX loopback monitor works diagnostically, while its production
  data-family admissions remain behind Gate C evidence.

Re-running the missing application E2E requires the downstream application
repository/revision that owns the inventory paths. Re-running the transient
source failures requires a new dated evidence run; it must not be treated as a
code-side reason to widen TLS, host, schema, chronology, or admission policy.
