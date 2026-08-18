# Local observation-time admission design

## Decision

The current local Asia/Shanghai clock is an observation fact, not a provider
fact. Operations approved by this design may place it in `observed_at`; they
must never copy it into `source_at`.

Two bounded operations qualify:

- Eastmoney `PostCloseFlows`: current China date, capture at or after 15:35,
  exact cardinality, continuous rank, ordered main-net amount, exact security
  identity and a source instant on the requested date for every row. Different
  row source instants are retained. The batch `source_at` is null unless every
  row instant is equal.
- TDX `T0Evidence`: exact TDX-only Quote, five-level order book, daily bars and
  five-minute bars for each requested instrument. All four evidence objects are
  retained. The result uses the current local observation time and keeps
  `source_at` null when the inputs do not share a provider source instant.

Neither operation is eligible for BR-033 strict realtime freshness. The design
does not weaken identity, cardinality, ordering, completeness, timeout, body
limit, transport or admission checks. It does not promote full-market ranking,
breadth, Level-2 auctions, futures delivery, CFETS or IMF.

## External contract

The existing append-only gRPC operation numbers and request schemas do not
change. `PostCloseFlows` now returns record schema
`magic.market.post_close_flow`; `T0Evidence` is a normal admitted handler and no
longer requires `allow_unadmitted=true`. Both responses are complete and
repository-admitted only after all operation-specific checks pass.

## Evidence and failure policy

Provider record evidence is immutable. Batch IDs bind the exact operation,
request date or input digest and local observation time. Missing source time is
represented by null. Provider, schema, identity, ordering, completeness, clock
gate and transport failures remain typed failures; no partial success is
returned.

