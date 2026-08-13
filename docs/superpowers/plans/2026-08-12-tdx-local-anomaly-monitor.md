# TDX 本地终端监听实施计划

Status: diagnostic foundation implemented; production admission remains pending
Design: `../specs/2026-08-12-tdx-local-anomaly-monitor-design.md`

## Objective

Deliver an optional Rust Windows monitor that needs no TDX path, endpoint,
Python or DLL configuration:

```text
find current-user/current-session TdxW.exe
  -> probe fixed http://127.0.0.1:17709/
  -> poll an explicit watchlist
  -> run deterministic per-family rules
  -> emit bounded typed events
```

The existing provider/router libraries remain unchanged. Production admission
is per data family and remains false until evidence closes its contract.

## Non-negotiable decisions

- Use only the official TQ-Local HTTP interface for local-terminal data.
- Never load/call vendor DLLs and never use Python in the runtime path.
- Do not accept a TDX installation path or TQ endpoint override.
- Discover exact `TdxW.exe` in the current user/session.
- Treat executable identity as provenance; use bounded HTTP schema health to
  decide compatibility after upgrades.
- Permit only reviewed read-only method enum variants.
- Disable proxy use and redirects; bound timeouts and bodies.
- Preserve unavailable fields as `None`; never fill amount/source time/count
  with zero or host observation time.
- Keep price, amount and volume inputs/rules/admissions independent.
- Do not add an inbound HTTP/WebSocket listener before its separate Gate A and
  `http-transports.tsv` row.
- Do not commit or push as part of this local development run.

## Task 1 — Core event contracts

Status: completed.

- Add validated stream generation, sequence and cursor types.
- Add observation/source continuity and explicit time basis.
- Add rule identity/revision/definition digest.
- Add exact input evidence and deterministic input/event digests.
- Add replay-stable `AnomalyEvent`, source status and `MarketEvent` union.
- Enforce `LocalTerminal` inputs and `LocalAnalysis` derived evidence.
- Reject Serde tampering and time/generation contradictions.

Acceptance:

- Core full-target tests, Clippy and formatting pass.
- No provider/transport dependency is introduced.

## Task 2 — Deterministic monitor kernel

Status: completed deterministic foundations and family-selective integration;
production thresholds remain unset pending shadow evidence.

- Keep all thresholds, windows, tolerances, cooldowns and limits explicit.
- Maintain independent price, cumulative-amount and cumulative-volume state.
- Accept family-selective observations: price monitoring must not require
  amount; volume monitoring must not require amount; missing required family
  data returns a typed result without mutation.
- Preserve optional source-record-count and cursor endpoints.
- Reset the affected family on its cumulative rollback or explicit reset.
- Accept explicit trading-date/session/midday reset signals; do not read wall
  clock/calendar inside the monitor.
- Produce deterministic Core event evidence.
- Keep all anomaly admissions false.

Acceptance:

- Tests cover price-only, volume-only, amount-only and missing-family inputs.
- No missing field is converted to zero.
- All monitor/composition tests and Clippy pass.

## Task 3 — Fixed TQ-Local Rust client

Status: two-method closed read-only client completed; both methods have bounded
local evidence and production admission remains false.

- Implement the exact origin `http://127.0.0.1:17709/` with no override.
- Implement closed `TqReadMethod` enum containing only `get_pricevol` and
  `get_market_snapshot`.
- Disable environment proxies and redirects.
- Inject positive connect/read/write timeout and byte limits; no defaults.
- Serialize calls until concurrency is measured.
- Validate content type, HTTP status, response ID, RPC `ErrorId`, instrument,
  decimal fields and exact response schema.
- Map `get_pricevol` `Now` to CNY/share price and `Volume` to cumulative lots;
  leave amount/source time/source-record count absent on that method.
- Request only `[Amount, Now, Volume, LastClose]` from `get_market_snapshot`;
  convert string `Amount` from ten-thousand CNY to CNY with checked exact
  decimal arithmetic and retain volume as lots. Source time and record count
  remain absent.
- Provide a diagnostic example that prints bounded provenance/latency and never
  prints a production admission marker.

Acceptance:

- Unit/integration fixtures cover request shape, host/path, redirect, timeout,
  connect failure, size bounds, content type, JSON, RPC, ID and schema failures.
- Windows Rust live probe succeeds against a running compatible TDX client.
- Source scan contains no account/trading method construction.

## Task 4 — Windows automatic discovery

Status: deterministic discovery and current-user/session implementation
completed; packaged Windows restart/upgrade live matrix remains pending.

- Keep a bin-only, non-published `magic-tdx-native-bridge --discover` helper.
- Enumerate only exact `TdxW.exe` in the current interactive session.
- Verify candidate owner equals current user.
- Revalidate name/session after opening the process and retain stable identity
  evidence where Windows permits.
- Bound ambiguous-process evidence.
- Return `discovered` with exit 0 for exactly one verified client; this means
  only “proceed to loopback health”, never data admission.
- Return typed nonzero absent/ambiguous/unsupported/failure states.
- Record executable path, architecture, digest and available version evidence.
- Remove all vendor-DLL layout, export and native-ABI fields/checks.
- Keep `--probe`/`--serve` unavailable; no market-data access in this helper.

Acceptance:

- No TDX produces a typed absent result.
- One current-user/session TDX produces a bounded discovered result.
- Multiple candidates are typed ambiguous and never guessed.
- No `LoadLibrary`, vendor exports, Python or data-DLL dependency exists.
- Native boundary compliance test passes.

## Task 5 — Automatic lifecycle service

Status: deterministic diagnostic lifecycle and one 12-cycle Windows E2E run
completed; packaged absent/ambiguous/exit/restart live acceptance remains
pending.

Create `magic-market-monitor-server` as a Windows leaf binary:

- Resolve the discovery helper relative to the server executable; do not search
  arbitrary `PATH` or accept a helper override in production.
- Accept only monitoring policy inputs: an explicit
  `EQUITY:SH|SZ|BJ:dddddd` watchlist, poll/retry limits, HTTP bounds, explicit
  rule parameters, snapshot and identity-recheck cadence, diagnostic cycle
  bound, event bound, output queue/shutdown bounds and slow-consumer policy.
- Start in discovery. With no terminal, remain waiting and do not start a data
  poller or caller listener.
- With one terminal, invoke the fixed TQ health/read path.
- Use a blocking worker boundary; never block an async executor thread.
- Start a new stream generation after successful health.
- Poll `get_pricevol` serially and build price/volume observations without
  amount fabrication. Poll `get_market_snapshot` through a separately paced,
  capacity-one amount worker so a cold snapshot cannot block the fast loop.
- Detect client exit and transport/schema failures, inject resets and return to
  waiting/backoff.
- Bound restart attempts, event line length and shutdown.
- Emit typed bounded four-byte big-endian length-prefixed JSON frames to stdout
  only; this is binary framing, not JSON Lines or an inbound listener.
- Use a bounded non-blocking stdout queue. The only policy is fail-closed
  `stop`; never drop frames or block the polling scheduler for a slow consumer.

Acceptance:

- Fake discovery/endpoint tests cover absent, ambiguous, ready, endpoint down,
  schema drift, child crash/hang, sequence gap, client exit/restart and shutdown.
- A Windows live run starts automatically with the installed running client.
- Current evidence: one bounded run completed 12 cycles/exit 0, warmed all
  three families, retained admitted=false and joined the snapshot worker.
- Killing and restarting TDX produces a new generation with no orphan process.
- No inbound network listener exists in this task.

## Task 6 — Version-aware health and provenance

Status: executable identity, generation provenance and schema-health foundation
implemented; unknown-version upgrade live matrix remains pending.

- Record executable identity from discovery.
- Retain numeric file/product version and version source, or a structured
  version-read failure; never infer a version from a path/display string.
- Define a version/schema observation record independent of data admission.
- Exercise known and unknown executable hashes against the same fixed health
  schema.
- Permit an updated executable when the implemented read schema is compatible;
  record it as new/unreviewed provenance.
- Fail closed on schema/unit/field drift.
- Re-run discovery at an explicit positive cycle cadence; a changed
  PID/session/creation identity resets windows and starts a new generation.
- Add golden response fixtures for every admitted method revision.

Acceptance:

- Unknown hash alone does not require user configuration.
- Compatible health succeeds; incompatible schema is typed unavailable.
- Event/source provenance identifies the stream generation and observed client
  identity without retaining raw payloads.

## Task 7 — Field evidence and per-family admission

Status: price/volume/amount deterministic schema and unit evidence exists;
reset/session/shadow closure is incomplete and all production booleans remain
false.

For each family independently:

1. document official or bounded empirical semantics;
2. add exact fixtures and live probe evidence;
3. prove unit, missing/null/error behavior and reset semantics;
4. update `admissions.tsv` in the same change;
5. enable repository admission only with a real end-to-end composition.

Order:

- price from `get_pricevol`;
- cumulative volume from `get_pricevol`;
- cumulative amount from independently paced `get_market_snapshot`; the
  installed vendor sample defines `Amount` as ten-thousand CNY, and conversion
  must be exact decimal multiplication by 10,000;
- source record count only after its definition is proved;
- source timestamp/freshness only after a source-owned timestamp is proved;
- price/volume/amount anomaly events after their input family is admitted.

Never infer amount units from display formatting and never treat snapshot
`ItemNum`, bridge sequence or provider tick counts as source record count.

## Task 8 — Calendar/session integration

Status: pending authority selection.

- Select and register an approved exchange calendar/session owner.
- Feed explicit trading-date/session/midday reset signals into the monitor.
- Cover holidays, auctions, midday break, close, clock skew and client restarts.
- Keep strict source freshness false until actual TQ source time is proved.

## Task 9 — Shadow performance evidence

Status: initial latency observations only.

- Run bounded watchlists through a full trading session.
- Record request latency percentiles, response sizes, CPU/memory, connection
  errors, terminal restarts, schema errors, poll drift and event rates.
- Test cardinalities up to documented/observed method limits without load-probe
  abuse.
- Select production poll/retry/queue/replay defaults only from results.
- Keep existing `ReplayLimits`, supervisor budgets and poll intervals injected
  until then.

Initial diagnostic evidence (not defaults): ten calls about 19–51 ms; Rust probe
about 12 ms on the current installation.

## Task 10 — Deployment and CI

Status: Windows-host diagnostic pair packaging implemented; Windows CI/live
lifecycle evidence and any later production promotion remain pending.

- Build and install server/discovery helper together only on a Windows host,
  without packaging vendor files; non-Windows packages omit both.
- Preserve executable bits on Unix scripts.
- Add Windows build/test job for discovery and lifecycle fixtures.
- Keep safe crates `unsafe_code=forbid`; keep the one discovery exception
  audited and checker-enforced.
- Document Windows-only runtime and typed unsupported behavior elsewhere.
- Never auto-start the installed server. Its Windows package status is
  `diagnostic/admitted=false`; production enablement remains behind Gate D.

## Task 11 — Full repository quality gates

Run locally before declaring development complete:

```text
cargo fmt --all -- --check
cargo test --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo doc --workspace --all-features --no-deps --locked
tools/docs/check_links.sh
tools/compliance/check.sh
all tools/compliance/test_*.py
cargo deny check
cargo audit
git diff --check
```

Where network/cache or platform policy blocks a command, record the exact
environmental blocker; do not relabel it as pass.

## Task 12 — `data-sources-inventory.md` validation

Status: partially executed; final pass required after development.

- Execute every distinct upstream provider probe required by the inventory,
  serially per source to avoid rate limits.
- Record exact command, date/session, result, counts and admitted/diagnostic
  state in the evidence report.
- Re-run known failed or changed probes after fixes.
- Keep provider probe status separate from application E2E status.
- Mark missing downstream `src/data_gateway/**`, calendar, search and aggregation
  paths as `blocked_missing_source`, not provider pass.
- Do not substitute public TDX TCP probes for the new local TQ-Local lifecycle.
- Include local TDX discovery, health, price/volume, failure and performance
  evidence as their own rows.

Known application-level blockers in this repository include general web search,
the downstream calendar authority, concept-board direct ranking and several
gateway/aggregation paths referenced by the inventory but absent from this
workspace. The final report must state these precisely.

## Final completion criteria

Development is complete only when:

1. a running compatible TDX client is discovered and used without TDX path,
   endpoint, Python or DLL configuration;
2. absence/ambiguity/exit/restart/schema drift are typed and tested;
3. price and volume can run while amount remains unavailable;
4. no production event can be constructed by flipping admission booleans alone;
5. all relevant unit/integration/Windows live tests and repository gates have
   recorded outcomes;
6. every inventory row has an evidence status, including honest blocked rows;
7. no Git commit or push is performed during this local development run.
