# Eastmoney Minute1 fund-flow semantics (2026-10-10)

## Scope and result

This primary-source-only research concerns the exact public Eastmoney `api/qt/stock/fflow/kline/get` route with `klt=1`, not daily flows, ETF subscriptions, or another vendor's product. Research checked at `2026-10-10T03:08:21Z`. Only official page HTML, referenced static JavaScript, official documents, and local contract files were read. No fund-flow/quote data endpoint was requested, no JavaScript was executed, and no credential, runtime, admission, allowlist, SDK, Git index, or frozen implementation was changed. This note is uncommitted research, not formal delivery or release evidence.

**Result: the crucial provider calculation semantics remain UNKNOWN.** The inspected official material identifies the five displayed net-flow fields and their presentation unit, but does not establish cumulative-versus-delta meaning, the Minute1 `f57` denominator, or the actual returned trading session. An unknown is an evidence gap, not a claim that Eastmoney does not support the behavior.

This note supplements `WINDOWS_FUND_FLOW_MINUTE1_CONTRACT_PREPARATION_20261010.json`. Its local counterpart is `target/runtime/resource-exhaustion-20261009/fund-flow-minute1-contract-preparation-20261010.json`, SHA-256 `f1a9ff92d971dd9e74519739bea40c6c08e612a8b54249e8807e683fd08a4d02`. The preparation binds HEAD `841e4ae7a9df62be0c4536fa9009c66089d282bf`, a dirty working tree, and 18 selected source files with fingerprint `5ab299b60ce4085ff03b151a6dd49e7f485015f512f230177591393a2e1971dd`; it is not a full-repository or release-binary identity. The adapter `crates/magic-eastmoney-rs/src/fund_flow.rs` independently still hashes to `050a04d1a32d51013e7d05d61fd8e22977245c71b63e24a11ed17879bb1f833b`.

## What first-party material proves

The [official stock fund-flow page](https://data.eastmoney.com/zjlx/002441.html) links the [stock script](https://data.eastmoney.com/newstatic/js/zjlx/stock.js). `getmkdata` calls the exact production `push2` route with `klt=1`, `lmt=0`, ordered `fields2=f51` through `f65` (lines 743–752, 1869–1889). `drawchart_fs` plots CSV positions 1–5 directly, in yuan, without summation/differencing (lines 1009–1224). The request order and series labels give:

| Source field | First-party displayed meaning |
| --- | --- |
| `f52` | Main net inflow |
| `f53` | Small-order net inflow |
| `f54` | Medium-order net inflow |
| `f55` | Large-order net inflow |
| `f56` | Super-large-order net inflow |

These are **client-consumption facts**, not an aggregation specification. `getPeriods` uses response `tradePeriods.periods`, generating minutes after `b` through `e` (lines 1011–1014, 1957–1969); actual boundary values, completeness, and timestamp meaning are unproven. [Official script](https://data.eastmoney.com/newstatic/js/zjlx/stock.js).

A second independently linked [official quote-page chart script](https://quote.eastmoney.com/newstatic/js/libs/quotemoneyflowchart0715.js), discovered in the HTML of the [official quote page](https://quote.eastmoney.com/sz002467.html), likewise uses the exact Minute1 route. Its `getKLData` consumer divides each returned amount by a presentation scale and rounds it, without cumulative summation or first-differencing. It does not request `f57`. This corroborates presentation behavior only; an unchanged raw value could already be either cumulative or delta. [Official quote chart script](https://quote.eastmoney.com/newstatic/js/libs/quotemoneyflowchart0715.js).

## Unresolved calculations

| Question | Status | Boundary |
| --- | --- | --- |
| Are `f52`–`f56` cumulative from the opening session? | `UNKNOWN` | No inspected owner definition states their integration/reset window. |
| Are they the net flow within one minute? | `UNKNOWN` | `klt=1` identifies a request interval; it does not alone define aggregation. |
| Does Minute1 `f57` use total turnover as denominator? | `UNKNOWN` | No inspected owner formula ties this exact field/route to that denominator. |
| Does its denominator cover one minute or the session to date? | `UNKNOWN` | No inspected owner formula establishes either window. |
| What are auction, lunch-break, correction, rounding, and zero-denominator rules? | `UNKNOWN` | No inspected route-specific specification establishes them. |

The Minute1 chart ignores CSV position 6. The script's **daily** table appends a percent sign there, consuming the distinct `getdkdata` daykline route (lines 1830–1910): daily display evidence, not a Minute1 formula. [Official stock script](https://data.eastmoney.com/newstatic/js/zjlx/stock.js).

The frozen project's parser maps position 6 to optional `main_ratio` with `Percent` units, preserves absent/sentinel values as missing, and performs no denominator calculation (`fund_flow.rs`, lines 175–176 and 238–247). That is the **project's contract**, not independent Eastmoney confirmation. Neither a plausible field name nor a daily formatter closes the Minute1 semantic gap. Missing ratios must not be fabricated from quotes, turnover, five net amounts, or daily ratios on the strength of this research.

## Source session versus market hours

The page calls its initial chart load without a trading-time gate. Subsequent chart refreshes use `setIntervalInTrade`. The inspected [official common script](https://data.eastmoney.com/newstatic/js/main.js), lines 142–154, gates browser-local time at 09:25–11:44 and 13:00–15:14 and checks `getDay() > 5`. It neither uses an exchange holiday calendar nor explicitly selects China time; Sunday is not excluded by that condition. These are frontend polling rules, **not source publication, timestamp, or trading-session guarantees**. [Official common script](https://data.eastmoney.com/newstatic/js/main.js).

For exchange-level context only, the [SSE official stock-investment page](https://one.sse.com.cn/onething/gptz/) describes weekday trading with holidays/announced closures excluded, opening auction 09:15–09:25, continuous auction 09:30–11:30 and 13:00–14:57, and closing auction 14:57–15:00, subject to suspensions and adjustments. Its separate after-hours description must not be silently folded into this endpoint. [SSE official stock-investment page](https://one.sse.com.cn/onething/gptz/).

The [SZSE 2026 trading rules](https://docs.static.szse.cn/www/lawrules/rule/trade/W020260424690713155663.pdf), section 2.3 on printed pages 6–7, independently give the same auction/continuous-auction intervals and exclude holidays and announced closures. Neither exchange document defines Eastmoney's fund-flow endpoint, its auction inclusion, reset, retention, delay, or minute labeling. [SZSE official trading rules](https://docs.static.szse.cn/www/lawrules/rule/trade/W020260424690713155663.pdf).

Consequently, the source's actual first/last minute, session date, lunch-break handling, last actual trading session returned on a weekend, and full-session coverage remain `UNKNOWN`. The webpage's “today” labels or 09:31/11:30/15:00 axis labels are not response observations. The project's `f51` to `source_at` normalization and local `observed_at` receipt clock do not prove freshness or resolve these questions. No “today” quote or inferred latest-session observation is supplied here.

## Search limits and reproducibility

The stock page's [official help link](https://stock.eastmoney.com/news/1423%2C20110101117172217.html) was followed. The web reader redirected to [the official finance URL](https://finance.eastmoney.com/a/20110101117172217.html) but exposed no relevant article definition. A direct PowerShell request refused an insecure redirect; that protection was not relaxed. The historical [Choice user-guide URL](https://choice.eastmoney.com/FileDownload/Guide/Choice%E9%87%91%E8%9E%8D%E7%BB%88%E7%AB%AF%E4%BD%BF%E7%94%A8%E6%8C%87%E5%8D%97.pdf) redirected to the Choice home page, not the guide. These are retrieval limits, not evidence that no documentation exists.

Targeted searches of `eastmoney.com`, `help.eastmoney.com`, `choice.eastmoney.com`, and `dfcfw.com` for this route/field and cumulative/minute/net-ratio formulas found no inspected owner-authored route-specific definition. Hosted user posts, crawler blogs, and third-party reports were not adopted as provider evidence. General “fund flow” descriptions and another product's calculations are not substitutes for this exact endpoint's specification.

The JavaScript assets were read as text over normal HTTPS, without execution. The following lengths and SHA-256 values identify the UTF-8 encoding of the returned text, **not independently retained original transport bytes or an upstream version guarantee**. No response-body archive was created by this task.

| Official static asset | UTF-8 bytes | SHA-256 |
| --- | ---: | --- |
| [Stock script](https://data.eastmoney.com/newstatic/js/zjlx/stock.js) | 84095 | `12bde3ac75cd2264a0d2b37a647c3099538dc4939ae3ccce34db3e9e612096e3` |
| [Common script](https://data.eastmoney.com/newstatic/js/main.js) | 40670 | `01964a1a32fd6eea3ef2ba71e58c713aaa2bc7407a407ee4652980a2feec0a14` |
| [Quote chart script](https://quote.eastmoney.com/newstatic/js/libs/quotemoneyflowchart0715.js) | 81526 | `5752bdae1f01a5c0ad0bbaf630074ba419f897199a615e26619f7d35836fcb70` |

## Decision boundary

Keep the existing preparation's unknowns explicit. Do not rename these observations to “one-minute delta,” sum them into a daily flow, difference them into deltas, derive missing ratios, declare a full/current session, or change admission from these static findings. A future owner specification must name this exact route/interval, formula/window, boundary/reset behavior, and revision. Separately authorized real-session controls would need identity-bound original responses, source session metadata and times, receipt times, hashes, and reproducible expected relations; samples alone would not prove a universal algorithm. None were run here.

This research does not close Mac's request-builder/decoder delivery gaps or H08 full daily historical eligibility, price-band, revision-publication, and PIT qualification gaps. Those remain separate **not-delivered/evidence-missing** boundaries, not a vendor-unsupported conclusion.
