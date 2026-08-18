# gRPC derived market-data products

This document fixes the version-1 external payload contracts for five composed
market-data products. They are append-only `MarketDataService` RPCs using the
common `QueryRequest` / `QueryResponse` envelope. The JSON below is carried in
`CanonicalPayload.data` with `schema_version=1` and
`content_type=application/json; charset=utf-8`.

The interfaces are present now so consumers can generate stable clients.
All five products have passed their deterministic composition tests, two live
probes and three serial live requests. `T0Evidence` is admitted as an exact
four-family evidence bundle using current local Asia/Shanghai observation time;
it does not claim source-time freshness when TDX omits source timestamps.

## Common rules

- Unknown JSON fields are rejected. Required fields are never defaulted.
- `InstrumentId` uses exact Core spelling, for example
  `{"exchange":"Shanghai","code":"000001","asset_class":"Index"}`.
- Prices, money, quantities and ratios are JSON numbers. Missing source values
  are JSON `null`; zero must not stand in for missing data.
- Dates are `YYYY-MM-DD`; instants include an explicit offset. Local fetch time
  must not be copied into `source_at`.
- `input_evidence` is a non-empty, source-order array of exact Provider
  evidence objects: `provider`, nullable `source_at`, `observed_at`, and
  `batch_id`.
- Every derived record carries `algorithm_id`, positive
  `algorithm_revision`, and a lowercase 64-character `input_digest_sha256`.
  The digest commits the complete normalized inputs in source order; it is not
  a claim that the source supplied a digest.
- Partial multi-family composition fails. It does not return a successful
  record with missing required families.

## `IndexQuotes`

- Operation enum: `OPERATION_INDEX_QUOTES = 56`
- RPC: `MarketDataService/IndexQuotes`
- Request schema: `magic.market.index_quotes.request`
- Record schema: existing `magic.market.quote`
- Intended Provider: Tencent only until another index contract passes its own
  admission.
- Capability state: admitted and runtime available when the fixed Tencent
  client can be constructed.

Request data:

```json
{
  "indices": [
    {"exchange":"Shanghai","code":"000001","asset_class":"Index"}
  ],
  "maximum_source_age_millis": 5000
}
```

`indices` contains 1 through 6 unique index identities. The freshness value is
positive and explicit; source timestamps are required for every admitted
record. Equity identities are rejected rather than re-labelled as indices.

On 2026-08-16, two live probes and a separate three-request serial probe each
returned all six configured indices with `Available` status, exact Tencent
record evidence and non-empty source times. Because the probe ran on Sunday,
the caller explicitly allowed three days and the records correctly retained
their 2026-08-14 source instants. A 5-second request on that same observation is
rejected as stale.

## `IntradayShape`

- Operation enum: `OPERATION_INTRADAY_SHAPE = 57`
- RPC: `MarketDataService/IntradayShape`
- Request schema: `magic.market.intraday_shape.request`
- Record schema: `magic.market.intraday_shape`

Request data:

```json
{
  "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
  "trading_date":null,
  "maximum_points":800
}
```

`trading_date=null` means the current source session and does not authorize the
service to invent a date. A non-null date selects a verified historical minute
contract. `maximum_points` is positive and at most 800.

One output record contains:

```json
{
  "instrument":{},
  "trading_date":"2026-08-14",
  "source_interval":"Minute1",
  "first_at":"2026-08-14T09:30:00+08:00",
  "last_at":"2026-08-14T15:00:00+08:00",
  "point_count":242,
  "open":18.18,
  "high":18.18,
  "low":16.97,
  "latest":16.99,
  "vwap":17.499821326930984,
  "cumulative_volume":2835626,
  "cumulative_amount":4962294835,
  "up_points":98,
  "down_points":127,
  "flat_points":17,
  "input_evidence":[],
  "algorithm_id":"magic.intraday_shape",
  "algorithm_revision":1,
  "input_digest_sha256":"0000000000000000000000000000000000000000000000000000000000000000"
}
```

`vwap`, cumulative volume and cumulative amount are nullable independently when
the source contract does not supply their required inputs. OHLC/latest,
point-count and direction counts are required and are computed only from the
complete ordered minute input.

On 2026-08-16, two live requests and a separate three-request serial run for
600396.SH and trading date 2026-08-14 each returned the same 242 regular-session
points from 09:30 through 15:00 and the values shown above. The source also
returned 15:06..15:30 post-close rows; they remain committed in the input
digest but are deliberately excluded from regular-session shape arithmetic.

## `T0Evidence`

- Operation enum: `OPERATION_T0_EVIDENCE = 58`
- RPC: `MarketDataService/T0Evidence`
- Request schema: `magic.market.t0_evidence.request`
- Record schema: `magic.market.t0_evidence`
- Provider policy: TDX only; no fallback or mixed-Provider bundle.

Request data:

```json
{
  "instruments":[{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}],
  "daily_bar_count":20,
  "five_minute_bar_count":48
}
```

The request accepts 1 through 8 unique A-share equities. Both counts are
positive and at most 800. Each output record contains exact `instrument`, one
`quote`, one `order_book`, echoed checked `daily_bar_count` and
`five_minute_bar_count`, ordered `daily_bars`, ordered `five_minute_bars`,
non-empty `input_evidence`, `algorithm_id=magic.t0_evidence`, positive
`algorithm_revision`, and `input_digest_sha256`. Every required family must
refer to the same instrument and one bounded capture; mixed Provider data is
rejected.

The formal handler performs the four TDX reads without fallback or
`allow_unadmitted`. Two live requests and a separate three-request serial run on
2026-08-17 for 600396.SH each returned one Quote, one five-level book, 20 daily
bars, 20 five-minute bars and four exact evidence objects. The response was
complete and repository-admitted. Its `observed_at` was the current local
Asia/Shanghai instant; Quote/book source times and the response `source_at`
stayed `null`. This is a production evidence bundle, not a BR-033 five-second
freshness signal.

## `OutcomeDailyBars`

- Operation enum: `OPERATION_OUTCOME_DAILY_BARS = 59`
- RPC: `MarketDataService/OutcomeDailyBars`
- Request schema: `magic.market.outcome_daily_bars.request`
- Record schema: `magic.market.outcome_daily_bars`
- Provider policy: TDX only; no routing or fallback.

Request data:

```json
{
  "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
  "through":"2026-08-14",
  "limit":20,
  "outcome_due_at":"2026-08-14T15:35:00+08:00"
}
```

`limit` is positive and at most 800. `outcome_due_at` is mandatory and must not
precede the end of `through`. One output record contains `instrument`,
`requested_through`, checked `requested_limit`, `outcome_due_at`, `bars` in
strict oldest-to-newest order,
the exact TDX `input_evidence`, `algorithm_id=magic.outcome_daily_bars`, positive
`algorithm_revision`, and `input_digest_sha256`. The newest bar must equal
`requested_through`; a short, discontinuous, earlier, or later source result
fails.

On 2026-08-16, two live requests and a separate three-request serial run for
600396.SH each returned exactly 20 oldest-to-newest TDX daily bars from
2026-07-20 through 2026-08-14, with the requested outcome due instant fixed at
2026-08-14T15:30:00+08:00 and one exact input evidence object.

## `UpperLimitPoolReview`

- Operation enum: `OPERATION_UPPER_LIMIT_POOL_REVIEW = 60`
- RPC: `MarketDataService/UpperLimitPoolReview`
- Request schema: `magic.market.upper_limit_pool_review.request`
- Record schema: `magic.market.upper_limit_pool_review`
- Provider policy: admitted Eastmoney limit-pool source plus deterministic
  composition only.

Request data:

```json
{
  "trading_date":"2026-08-14",
  "per_pool_limit":1000
}
```

The limit is positive and at most 1000 and applies independently to `Upper`,
`Broken`, `Lower`, and `PreviousUpper`. One atomic output record contains the
exact `trading_date`, four source-order arrays `upper`, `broken`, `lower`, and
`previous_upper`, their four counts, nullable `maximum_streak`, non-empty
`input_evidence`, `algorithm_id=magic.upper_limit_pool_review`, positive
`algorithm_revision`, and `input_digest_sha256`. It contains facts and source
labels only; it must not add buy/sell recommendations or strategy scores.

On 2026-08-16 the deterministic handler test proved one atomic record and
fail-closed family validation. Two live requests and a separate three-request
serial load against trading date 2026-08-14 each returned `Upper=63`,
`Broken=19`, `Lower=10`, `PreviousUpper=59`, `maximum_streak=5`, four exact
input evidence objects and a lowercase SHA-256 input commitment. A
source-verified empty pool is represented as an empty array with its original
evidence; an ordinary empty or truncated batch is not accepted.

## Current capability state

All five derived products are admitted. `T0Evidence` accepts the normal request
path, uses TDX only, returns `complete=true` and does not require an opt-in flag.
Its local `observed_at` must never be copied into nullable provider `source_at`.

The rebuilt Windows service was then exercised through its real mTLS + Bearer
endpoint at `10.211.55.3:50051`; the address is deployment evidence, not a
portable endpoint default. On 2026-08-18 the external capability registry
reported 60 operations, 56 admitted and four blocked. A formal external
`T0Evidence` request returned `complete=true`, `ADMITTED`, a current local
`+08:00` `observed_at`, and `source_at=null`. This confirms that deployment uses
the local observation clock without converting it into provider source time.
