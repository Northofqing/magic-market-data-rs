# TDX lifecycle acceptance evidence — 2026-07-27

This record captures the live acceptance boundary for the normalized TDX
security-metadata and corporate-action gateways. It is evidence for the
observed run only; it is not a promise that every public TDX node will remain
available.

## Commands and environment

The lifecycle rerun completed around `2026-07-27T13:15:00+08:00`
(`Asia/Shanghai`). The raw protocol inventory used:

```bash
TDX_LIFECYCLE_RAW_ONLY=1 \
  cargo run -p magic-tdx-rs --example live_probe --locked --offline
```

That command exited zero. The normalized lifecycle assertions were then
executed by:

```bash
cargo run -p magic-tdx-rs --example live_probe --locked --offline
```

Cargo's `--offline` flag kept dependency resolution offline. The compiled probe
still used the TDX network protocol, as intended. Both runs selected:

```text
杭州联通J2 (60.12.136.250:7709)
```

The complete multipurpose probe exited one because an unrelated intraday-bar
clock assertion observed a future five-minute timestamp and a later block-index
request timed out. The lifecycle section itself passed every exact assertion:
two 2024 records with exact terms and a complete request-bound empty 1900
result. This evidence therefore admits the lifecycle Gateway only; it does not
claim that the unrelated full probe passed.

## Normalized security metadata

The Gateway returned exactly the three requested identities after validating
the finance-response market/code header and the Gregorian listing dates:

| Instrument | Name | `listed_on` |
| --- | --- | --- |
| `Shanghai/600396/Equity` | 华电辽能 | `2001-03-28` |
| `Shenzhen/000001/Equity` | 平安银行 | `1991-04-03` |
| `Shanghai/600519/Equity` | 贵州茅台 | `2001-08-27` |

The normalized metadata remains intentionally incomplete where TDX does not
prove a versioned price-limit rule, authoritative board identity or auditable
source timestamp. A valid listing date does not promote those unrelated fields.

## Normalized corporate actions

The raw response contained 45 rows. Before applying either requested date
range, the Gateway parsed and validated all 45 rows, including identity,
framing, dates, category-specific schema, finite numbers, non-negative
capital-structure values and positive warrant terms:

```text
category_histogram={1: 30, 2: 7, 3: 1, 5: 5, 9: 1, 14: 1}
```

The formal contract covers every TDX XDXR category from 1 through 14. Categories
2 through 10 preserve all four provider-native capital-structure values
(`tradable_before`, `tradable_after`, `total_before`, `total_after`). Categories
13 and 14 preserve exercise price and provider-native source quantity.
Category 11 retains the source's broader “扩缩股” meaning as
`CapitalRescaling`; it is not narrowed to a split. The upstream decoder leaves
the physical unit of the capital quantities, warrant quantities and category
11/12 `suogu` value unresolved, so these values carry
`UnverifiedSourceUnit::ProviderNative` and must not be treated as shares, lots,
per-ten-share values or adjustment ratios. The category table and decoder
layout are also visible in
[rainx/pytdx `get_xdxr_info.py`](https://github.com/rainx/pytdx/blob/master/pytdx/parser/get_xdxr_info.py).

For `Shanghai/600519/Equity` and the inclusive range
`2024-01-01..2024-12-31`, the Gateway returned a complete batch with two
implemented distribution actions:

| `effective_on` | Category | Status | Normalized cash/share |
| --- | --- | --- | --- |
| `2024-06-19` | `Distribution` | `Implemented` | `30.8760009765625` |
| `2024-12-20` | `Distribution` | `Implemented` | `23.882000732421876` |

Both records carried `ProviderId::Tdx` and the exact batch identity from the
response provenance. Their `source_at` value was `None`: the XDXR packet has no
verified supplier timestamp, so the adapter preserved acquisition time as
`observed_at` without relabelling it as source time.
The response's explicit `admission_as_of` was `2026-07-27`; Core and Router
reject later coverage or effective dates instead of trusting a
Provider-selected future boundary. The specialized lifecycle Router owns one
immutable date for the entire failover chain, accepts only its sealed
response-validating source adapter, and retains the date in the selected route
outcome. Individual Providers cannot register a later fallback boundary.

For the same instrument and the inclusive range
`1900-01-01..1900-12-31`, the Gateway returned zero records with complete
quality and request-bound provenance:

```text
corporate_actions_verified_empty=true
```

This is a verified empty result after a complete response was parsed and
validated, not an `Unsupported` response or a transport failure converted into
an empty batch.

## Explicit boundaries

- Beijing normalized security metadata remains `Unsupported` because the
  required `market=2` security-list endpoint is not available on the verified
  public nodes.
- Beijing normalized corporate actions are rejected as `Unsupported` before
  transport. They are not retried against a Shanghai or Shenzhen market code.
- XDXR categories 1 through 14 are checked before range projection. Any category
  outside that protocol table, even outside the requested range, returns
  `InvalidData`; malformed rows are never hidden by date filtering.
- Provider-native capital-structure and warrant quantities have an explicitly
  unverified physical unit. They preserve source evidence but are not admitted
  for share-count arithmetic or adjustment-factor calculations.
- Missing TDX source timestamps remain explicit and cannot satisfy a
  source-time freshness gate.

The transport-independent Beijing lifecycle boundary is also covered by:

```bash
cargo test -p magic-tdx-rs --test lifecycle_provider --locked --offline
```
