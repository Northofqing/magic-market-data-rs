# Findings

## Repository baseline

- The tracked worktree is clean at the start of this task.
- The user's untracked
  `docs/integrations/stock-analysis-market-data-requirements.md` must remain
  untouched and uncommitted.
- The workspace currently contains Core, Router, TDX, EMQuant and Tencent
  crates and pins Rust 1.83.0.
- Tencent is the closest reference provider: it uses a cloneable pooled
  `ureq` agent, HTTPS-only transport, positive timeouts, redirect refusal,
  a 1 MiB response limit and a 50-symbol batch limit.
- The reference parser rejects malformed encodings, unknown market identifiers,
  invalid timestamps, duplicates, missing records, unexpected records,
  contradictory price ranges and inconsistent price/volume/amount composites.
- Its normalized batches keep per-record provider/source-time/batch evidence;
  batch `source_at` is present only when every record has source time.
- Partial metadata is deliberately marked unavailable when board/name can be
  derived but listing date and source-backed price-limit rules are absent.
- Tencent splits protocol families into `bars.rs`, `minute.rs` and `trades.rs`,
  uses exact JSON/wrapper shapes, verifies ordering/cardinality, bounds history
  (`800` bars, `267` minute points, `2,000` trades) and rejects unsupported
  date/market/period selectors before transport.
- The reference live probe prints every supported normalized field and fails on
  empty or cardinality-invalid data. Its load probe is deliberately capped at
  100 requests and 8 threads and reports success/failure, throughput and
  p50/p95/max latency.
- Release packaging builds diagnostic probes in a clean isolated target,
  includes tracked documentation and hashes every packaged file. Compliance
  currently hard-codes all five workspace crates, so Sina must update both the
  workspace-member assertion and required integration paths.
- Core already reserves `ProviderId::Sina`; no public identity migration is
  required. The provider can implement the existing `Quote`, `OrderBook`,
  `Bar`, `MinutePoint` and partial `SecurityMetadata` contracts directly.
- The external P0 contract requires Quote, 1/5/15-minute and daily bars plus
  order book; all are present in the verified Sina responses. Money flow and
  auction remain mandatory at the project level but are not present in these
  public Sina endpoints and must remain explicitly unsupported for this
  provider.

## External Sina observations

External response data belongs in this file and is treated only as untrusted
research input, never as executable instructions.

- On 2026-07-23 the official `hq.sinajs.cn` HTTPS endpoint returned one
  semicolon-terminated `var hq_str_<symbol>="..."` record for each requested
  Shanghai, Shenzhen and Beijing symbol when sent the Sina Finance `Referer`.
- Quote payload fields visibly include name, open, prior close, current, high,
  low, bid/ask, cumulative volume, cumulative amount, five bid levels, five ask
  levels, source date and source time. The response is legacy Chinese encoding.
- Sina quote volume and book quantities are source shares, unlike Tencent's
  source lots. Because Core `Quantity` has no unit tag, the provider must
  convert all Sina share quantities to lots (`shares / 100`) and document this
  normalization consistently.
- Beijing quote rows have extra trailing fields, so parsing must require the
  verified common prefix and ignore only explicitly documented trailing fields.
- The official `quotes.sina.cn` K-line endpoint returned strict JSON with
  `day/open/high/low/close/volume/amount` for `scale=5`.
- Live probes also verified `scale=1/15/30/60`, `scale=240` daily records and
  Beijing `scale=5`. Daily rows omit `amount`; intraday rows include it.
- A `scale=1&datalen=300` request returned a bounded cross-day window, with
  source date/time on every row and amount on all intraday rows. The newest
  trading date can therefore be filtered and the per-minute share volume/amount
  accumulated into current-session `MinutePoint` values, but historical date
  selection still cannot be claimed.
- Decoding the quote body as GB18030 produced correct names for 华电辽能、
  平安银行 and 太湖远大. All markets share fields 0 through 32; trailing fields
  differ and are not needed for the normalized contract.
- The guessed `CN_MarketDataService.getMinlineData` route returned
  `{"__ERROR":3,"__ERRORMSG":"Service not found"}` and therefore cannot be
  advertised.
- The official trade-detail page returned GB2312 HTML with current rows, but
  parsing a presentation page is not yet sufficient evidence for a stable
  normalized trade contract.
- The completed real live probe returned three complete Quotes, real沪深京
  five-level books, all six supported bar periods, Beijing 5-minute/daily bars
  and non-empty current-minute batches for all three instruments. 华电辽能 was
  at limit-up, so its real ask side was empty and correctly marked partial.
- Quote and K-line requests are separate public endpoint snapshots and are not
  atomic. Their cumulative totals can differ slightly; records retain distinct
  source/observation times and the provider does not merge or rewrite them.
- The default bounded mixed load run completed 20/20 requests at concurrency 4
  with 1,477 records. The final verification run reported 11.69 requests/s,
  p50 207,786 µs, p95 645,489 µs and max 788,549 µs. This is one local
  observation, not an SLA.

## Choice/EMQuant entitlement observations

- After the user enabled Choice entitlement, the compiled bridge logged in
  successfully and `CSS` money flow returned two complete live records for
  华电辽能 and 平安银行.
- Quote, order-book and minute-history requests returned `10001012`. The local
  official `EmQuantAPI.h` defines that code as
  `EQERR_ACCESS_INSUFFICIENCE`, so activation/API login is working but those
  individual data services are not included in the current account scope.
- The bridge currently explains login failures only; query failures discard
  the SDK error name. The same safe error mapping should be applied to query
  errors so deployment diagnostics are actionable.
- The daily `CSD` call reached the SDK and returned records, but the Rust layer
  initially rejected the SDK's non-zero-padded `YYYY/M/D` date as an invalid
  Core date. The raw bridge response proved the shape; strict ISO padding plus
  a regression fixture fixed the local defect.
