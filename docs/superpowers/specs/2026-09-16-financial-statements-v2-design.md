# FinancialStatements v2 fiscal-period preservation

Date: 2026-09-16

## Problem

The Hithink Fuyao financial response publishes both `fiscal_year` and the
provider-native `fiscal_period` label (`Q1`, `H1`, `Q3`, `FY`). The adapter
validated `fiscal_period` but discarded it while constructing
`FinancialStatement`. A client could see `report_period`, but could not retain
the original period label when deciding whether an actual EPS value was
comparable with a full-year consensus estimate.

## Gate A decision

Preserve the provider-native label in Core and publish it only through
`magic.market.financial_statement` schema version 2. The existing request and
record version 1 remain accepted and byte-shape compatible: v1 projection omits
`fiscal_period`. A caller selects v2 by sending the existing
`magic.market.financial_statements.request` payload with `schema_version=2`.

The v2 record adds:

```json
{"fiscal_period":"Q1"}
```

The value is optional because not every admitted Provider publishes an exact
native label. It is source identity, not an inferred claim that every numeric
line is cumulative or single-quarter. Consumers may safely compare a Fuyao
`FY` actual EPS with a same-issuer, same-fiscal-year annual estimate; other
period-basis comparisons remain fail-closed unless separately proved.

## Alternatives rejected

- Adding the field to v1 would silently mutate a frozen payload contract.
- Deriving `FY`/quarter labels from `report_period` would replace source
  evidence with a calendar guess.
- Adding recent reports and target prices to `Consensus` would mix three
  existing independently evidenced operations (`Consensus`,
  `ResearchReports`, `TargetPrices`) and obscure batch lineage.

## Trade-offs

The append-only v2 projection keeps old clients working and gives new clients
the missing source fact. It adds one version branch to the service and requires
clients that need the field to opt into v2. Sina records may return
`fiscal_period=null`; callers must not substitute a derived label.

## Verification seam

- Fuyao normalization retains the exact source label.
- gRPC v1 omits the field and retains schema version 1.
- gRPC v2 includes the field and returns schema version 2.
- Invalid payload versions are rejected rather than defaulted.
