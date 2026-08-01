# Eastmoney Provider Top-N Ranking Design

**Date:** 2026-07-29
**Status:** Superseded in part by the 2026-08-01 settled-date capture amendment
**Rules:** BR-009, BR-010, BR-011, BR-021, BR-033, BR-034

## 1. Problem

The existing `MarketRankings` contract is intentionally a complete-universe
contract. It requires complete pagination, exact Shanghai/Shenzhen/Beijing
coverage, unique identity, complete metric coverage and one atomic source-time
context. Live probes on 2026-07-27 and 2026-07-29 proved that the Eastmoney
`clist/get` response cannot satisfy that contract:

- the source caps each page at 100 records, so a full A-share snapshot spans
  approximately 56 independently moving requests;
- the complete universe contains securities with missing `f10` and `f62`;
- per-security `f124` values differ and therefore cannot be promoted to one
  provider-native full-market source instant.

Those facts must continue to reject `MarketRankings`. They do not invalidate
the narrower source fact that one response is a provider-ordered Top-N page.
The downstream application needs that narrower fact for post-close volume-ratio
and main-net-inflow discovery.

## 2. Decision: dual contracts

The complete-universe contract remains unchanged and unadmitted.

A new `ProviderTopNRankings` contract represents exactly one provider response
page. It is not a full-market snapshot and must never be consumed as market
breadth or coverage evidence.

The new contract carries:

- metric kind and unit;
- contiguous source-response ordinal assigned locally after preserving the
  exact provider response order;
- exact instrument code and source-supplied name;
- finite source value;
- per-security `latest_trading_date` from the provider's `f297` field;
- request-bound filter identity and `provider_declared_total`; the filter
  identity is caller/adapter input and is not source-supplied metadata;
- exact inspected/returned row count, not a fabricated covered-universe count;
- one deterministic, collision-resistant batch ID binding metric, trading
  date, exact limit, filter identity, canonical response content and one
  post-response `observed_at`;
- batch provenance with no `source_at`, because neither `f297` nor `f124`
  proves when the ranking metric or page was produced.

The type name, fields and documentation must use “provider Top-N”. It must not
use “full-market”, “complete coverage”, or a synthetic coverage ratio.

## 3. Request and admission

`ProviderTopNRankingRequest` binds one supported metric, one exact China trading
date and a positive limit no greater than the source's proved one-page cap of
100.

The first Eastmoney production slice is post-close only. The observation-date
rules below are amended by
`2026-08-01-eastmoney-provider-topn-settled-capture-design.md`:

- the request date must not be later than the current China calendar date;
- acquisition must start no earlier than 15:35 China time;
- `observed_at` is captured only after the complete single response has been
  read. Same-date capture requires 15:35 or later; a later calendar-date
  capture is allowed only when every row still identifies the requested date
  in `f297`. Acquisition and completion must not cross capture-date midnight;
- the source response must declare a non-zero row total for the exact filter
  and return exactly `min(limit, provider_declared_total)` rows in one page;
- every selected row must contain the requested metric, instrument identity,
  source name and a valid `f297` latest trading date;
- every selected `f297` is retained as `latest_trading_date`, must equal the
  requested date and must not be in the future relative to the
  injected/current China clock;
- `source_order_ordinal` is locally assigned only after preserving provider
  response order; values must already be descending; equal values do not imply
  a provider tie rank or a complete cutoff set;
- identities must be unique and valid A-share identities;
- volume ratio is non-negative; main-net inflow may be positive, zero or
  negative;
- any transport, protocol, identity, ordering, time or evidence failure rejects
  the whole page.

Missing metrics outside the selected response page are unknown and are not
invented, counted or interpreted. A selected row with a missing metric rejects
the entire requested Top-N page.

Volume-ratio and main-net-inflow capabilities live in a new independent
`ProviderTopNRankingCapabilities` type. The existing
`MarketRankingCapabilities` and `SignalCapabilities.market_rankings` remain
false and are never enabled by Top-N admission. The formal trait remains
`Unsupported` until the corresponding deterministic suites and a bounded
post-close live probe pass. Diagnostic access stays explicitly named and
cannot be mistaken for admission; a Router source is registered only for a
metric whose Top-N capability is true.

The provider-neutral Core trait is only the acquisition seam and exposes no
provider-owned identity/capability metadata. The public capability value and
generic validator are validation inputs, not admission authority. The trait is
not an admission witness because downstream code may implement public traits.
The production route therefore lives in
`magic-market-composition`. Its zero-argument constructor creates the
production `EastmoneyClient` internally, derives provider ID/source
identity/capabilities from the concrete Eastmoney implementation, owns
Eastmoney error classification, exposes neither client/transport injection
nor generic registration, and revalidates every returned batch. A
downstream-local transport or wrapper cannot enter this route while claiming
Eastmoney's identity, source name or capabilities.

`magic-market-router` remains provider-neutral and depends only on Core.
`magic-market-composition` is the deliberate dependency join: it depends on
Core, Router and Eastmoney without introducing a reverse dependency into
either public contract crate.

## 4. Router and downstream boundary

The composition crate gets a distinct
`EastmoneyProviderTopNRankingRouter`; it does not reuse the
complete-universe `MarketRankingRouter`. Its dedicated source applies Core's
validator, which rechecks the exact request, admitted metric capability,
provider, cardinality, batch, identity, metric/unit, descending order,
continuous source-response ordinal, every `latest_trading_date`, and the
post-response `observed_at`. It requires the exact `+08:00` offset and, for a
same-date capture, a time at or after 15:35. It admits a later observation date
and rejects a cross-midnight completion. The contract
has no intraday `source_at` and must not be passed through the generic realtime
freshness policy.

The composition route also owns an independent Asia/Shanghai future-date gate.
Production construction installs a China-date clock which `route` evaluates
for every request; the private deterministic constructor injects a clock only
for unit tests. `route` rejects a future request before any provider call. A
past request may reach the current snapshot source, but succeeds only when
every returned `f297` proves that exact requested date. Re-evaluation on every
call means a long-lived route cannot retain authority for a future date.
Request and clock failures use a new composition-specific error type that wraps
the unchanged generic `RouterError`, preserving the existing public enum API.

Downstream consumers may describe the result as:

> Eastmoney single-response provider-ordered Top-N with the requested metric
> present in every returned row.

They must not use the result to calculate market breadth, universe coverage,
rise/fall counts or any full-market statistic.

### 4.1 Production composition live probe

Release evidence must include a bounded executable owned by
`magic-market-composition`, because calling the provider trait directly does
not prove the non-forgeable production composition boundary. The executable:

- constructs `EastmoneyProviderTopNRankingRouter` only through its
  zero-argument production constructor;
- exposes no client, transport, source identity or capability injection;
- defaults the request date to the current Asia/Shanghai calendar date and
  accepts only a valid positive limit within the proved one-page cap;
- accepts only the fixed `all` metric plan and routes `VolumeRatio` and
  `MainNetInflow` exactly once each, continuing to the second metric if the
  first fails;
- prints the selected provider, provenance source, response observation time,
  absent/present source time, record count and provider-declared total for
  each admitted metric;
- prints one final failure count and exits non-zero when either metric fails.

The executable is an operator probe, not a new library API. Its deterministic
tests cover input bounds and output field presence without performing network
I/O. The live run remains a separate, explicitly invoked Gate D action.

## 5. Failure modes

| Failure | Result |
| --- | --- |
| same-date capture before 15:35 or future request date | invalid request |
| capability not admitted | explicit `Unsupported` |
| transport/protocol failure | whole request fails |
| selected metric/name/code/time missing | whole request fails |
| source order, duplicate identity or cardinality mismatch | whole request fails |
| mixed selected latest-trading dates or future date | whole request fails |
| capture predates request or crosses capture-date midnight | whole request fails |
| mixed/missing selected `f297` dates | whole request fails |

`f124` is a per-security quote/update field and is not promoted to the ranking
metric's source time. The public page proves `f297` only as each selected
security's latest trading date. Neither field becomes batch `source_at`;
post-close admission is a requested-trading-date contract based on exact
`f297` evidence and the post-response `observed_at`, not a realtime freshness
claim. Intraday/realtime Top-N remains unadmitted.

No result is converted to an empty batch and no alternative ranking family is
used as a substitute.

The batch ID uses a versioned Top-N namespace, explicit metric/date/limit,
SHA-256 of the admitted filter identity and SHA-256 of canonical response JSON.
Canonical JSON sorts object keys, length-prefixes every scalar/container
boundary and preserves array/source order. This makes whitespace and object-key
order irrelevant while changes to normalized response content produce a
different identity. The identifier remains deterministic and uses neither
randomness nor a fabricated provider `source_at`.

## 6. Validation

```bash
cargo fmt --all -- --check
cargo test -p magic-market-core --test provider_top_n_rankings --locked
cargo test -p magic-eastmoney-rs provider_top_n_rankings --locked
cargo test -p magic-market-composition --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
bash tools/compliance/check.sh
bash tools/docs/check_links.sh

MAGIC_EASTMONEY_LIVE_OPERATION=provider-topn-rankings \
MAGIC_EASTMONEY_TOPN_DATE=<current-Asia/Shanghai-date> \
MAGIC_EASTMONEY_RANKING_KIND=all \
cargo run -p magic-eastmoney-rs --example live_probe --locked

MAGIC_COMPOSITION_TOPN_DATE=<current-Asia/Shanghai-date> \
MAGIC_COMPOSITION_TOPN_LIMIT=20 \
MAGIC_COMPOSITION_TOPN_KIND=all \
cargo run -p magic-market-composition \
  --example provider_top_n_live_probe --locked
```

The deterministic suites must additionally prove:

- Top-N records cannot enter the complete-market or breadth Router;
- complete-market capabilities stay false after Top-N admission;
- Top-N evidence has no intraday `source_at`;
- Provider and composition route both reject pre-15:35, wrong-date,
  cross-midnight and
  mixed/missing `latest_trading_date` responses;
- an unadmitted metric returns `Unsupported`;
- Provider/composition failures never become empty success;
- future self-consistent requests are rejected by the Router before provider
  I/O; older requests succeed only when current source `f297` still matches;
- provider/source/capability ownership cannot be supplied by a downstream
  caller;
- the same observation time cannot collide across metrics or distinct
  normalized response contents, while semantically equivalent JSON formatting
  retains the same batch identity;
- missing rows outside the one response and mixed `f124` values never produce
  a coverage or atomic-snapshot claim.

The live evidence must show both metrics separately. One passing metric never
admits the other. The operator supplies the requested settled trading date at
run time and records that exact date in the evidence document; the reusable
command must not hard-code an expired date.

## 7. Rollback

Revert the isolated upstream PR. The old complete-universe capability remains
false throughout, so rollback cannot silently re-enable a weaker path.
Downstream stays on explicit capability-unavailable behavior until it pins the
released immutable revision containing the admitted Top-N capability.
