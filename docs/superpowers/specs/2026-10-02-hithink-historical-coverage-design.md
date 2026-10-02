# Hithink historical-bar quality and observation coverage

## Gate A: scope and authority

The user's existing data-repair request authorizes local source repair and
public handoff to Mac Codex. Mac's technical review of Windows evidence batch
8 specifies the implementation below. This records that technical scope, not
a new human grant for deployment, credentials, network/TLS changes or authority
admission. A denied listener start must not be bypassed.

Keep D14/D17/D20 authoritative coverage and R08 automatic Confirmed fail closed.
No provider admission, HTTP registry, Core record, protobuf or default routing
change is proposed. The new SHA-256 dependency is non-HTTP and already exists
in the workspace/lockfile. No downstream path dependency is added.

## Behavior and public seams

1. Exercise `HistoricalBars::historical_bars` with injected external HTTP
   fixtures. Validate every received row before sorting and caller limiting.
   Preserve existing ascending output and most-recent-row selection. Actual
   local deletion must create a nonempty QualityReport issue via
   `DataBatch::best_effort`; no deletion keeps existing strict quality.
2. Add `HithinkClient::historical_bars_with_coverage`, sharing exactly one
   request/normalization with the existing trait. Return a batch plus closed,
   observation-only metadata. This is not an expected session calendar.
3. Hithink's HistoricalBars request schema version 1 keeps its record shape.
   Explicit version 2 returns one `magic.market.historical_bars.coverage`
   version-2 JSON envelope, including the original command request ID and
   SHA-256 of its exact request payload bytes, the decoded request, and the
   Provider outcome. Other historical Providers remain version 1 only.

The Provider coverage contains validated source row count, returned row count,
actual caller-limit truncation and a validated-response flag. Source exhaustion,
authority-calendar coverage and missing-date reasons are `Unknown`; source
revision and historical publication time are `NotProvided`; PIT is not
certified. Even no truncation and an empty source row set cannot promote these.

Native identity preserves only actual `thscode`, `interval`, upstream
`request_id`, `timestamp`, and the presence/value of `adjust` (Absent, Null or
Value). No request start/end, exchange or asset class is fabricated as a native
echo. The successful response receipt hashes the actual transport body before
JSON decoding, with byte length and validated final URL. It is not a hash of
reserialized JSON, HTTP headers or HTTP/2 wire bytes. No API key is exported.

Existing source-order behavior is deliberately retained: unordered source rows
are sorted, not rejected. Duplicate dates, identity/context conflicts, invalid
rows, range violations and timestamp conflicts still reject atomically before
limiting, including an invalid older row that would otherwise be discarded.

## Plan / Gates B through D

- First record a failing multi-row limit-1 public-trait test, then implement the
  minimal quality fix and verify no-deletion and atomic failure controls.
- Test the coverage public method with noncanonical raw JSON bytes, absence/null
  native fields and empty rows; implement only the evidence actually observed.
- Test version dispatch, v1 compatibility, v2 request binding, payload bounds
  and pre-I/O rejection through the composition query seam.
- Run relevant locked tests, fmt, Clippy, compliance and documentation checks.
- Deliver exact source inputs, patches, self-contained archive and test logs to
  the authorized shared directory. Report candidate status only: source/testing
  does not prove a new deployed Health or RPC identity.
