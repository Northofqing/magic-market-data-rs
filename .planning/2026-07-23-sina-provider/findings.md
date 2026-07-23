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
