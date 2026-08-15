# TDX Production Observation Admission Design

**Status:** Gate A approved by the user on 2026-08-15

## Decision

Promote only three source-backed TQ-Local observation families to production:
current price (`Now`, CNY/share), cumulative volume (`Volume`, lot), and cumulative
amount (`Amount`, exact ten-thousand-CNY to CNY conversion). The fixed endpoint,
read-only method allowlist, A-share universe correlation, bounds and generation
reset behavior do not change.

`source_timestamp` and `source_record_count` remain absent. Production responses
make an observation-time claim only. Strict freshness, OHLC/previous close,
Level-2 and every account/trading method remain outside the admitted scope.

## Event boundary

The monitor emits repository-owned per-family admission markers. The Agent may
promote the envelope only for `observation` when an admitted price or volume is
present, or `snapshot_observation` when an admitted amount is present. It rejects
an attempted admitted LocalAnalysis event. The gRPC server parses the bounded
canonical JSON again and permits admitted events only for those two schemas and
matching instrument identities.

Listener status advertises the three exact admitted families. Anomaly events
remain UNADMITTED because the current thresholds are operator examples and no
approved exchange-calendar authority is integrated into this leaf service.

## Evidence

Existing evidence includes 19 compatibility probes, a 6,000-cycle two-instrument
run and a 3,000-cycle 16-equity run with 48,212 bounded frames, explicit timeout
recovery and independent anomaly windows. On 2026-08-15 three additional serial
price/volume reads and three serial snapshot reads succeeded against the running
official client. The production decision is recorded per family in
`docs/integrations/admissions.tsv`.
