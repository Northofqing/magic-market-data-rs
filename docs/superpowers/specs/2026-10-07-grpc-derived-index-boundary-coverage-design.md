# Registered derived requests and index-route behavior coverage

Date: 2026-10-07

## Gate A scope and previously agreed seam

Continue the standing source repair at `OperationRegistry::execute`, the
existing agreed seam in the handler and upstream-boundary coverage designs.
Use the real `registry_with_tencent` factory and public Tencent transport
injection. Existing BR-045, BR-050 and `grpc-derived-products.md` contracts
remain unchanged. No new factory, private projection seam, architecture,
dependency, provider admission, endpoint or runtime permission is introduced.

Exact 841e4ae original CI 37594261090 measured overall 69679/81600 (85.39%,
passes 80) and critical 35597/39819 (89.40%, fails 95). Its original checker,
logs and JSON are immutable. The 2232-line deficit assumes the same denominator;
test counts are not a prediction of coverage gains or business completeness.

## Behavior slices

Add a separate external test module; preserve every original test and source
file. Add and execute one behavior before adding the next. Pin each operation's
actual registered provider. Only injected external Tencent responses may be
used for successful source calls; no valid TDX or Eastmoney request is sent.

Exercise exact IndexQuotes output, source units and identity; whole-response
payload refusal; stale or incomplete source rejection; closed route traces
for acquisition and protocol failures; invalid index identities and bounds;
and the existing derived requests' cardinality, timestamp, session deadline,
unknown-field and output-family bounds. Expected results are literal contract
examples, not a second parser or assertions computed from private helpers.

Success fixtures permit a caller-selected very large freshness interval so
fixed historical source instants remain literal. This is neither a live source
freshness claim nor a policy change. Tight freshness refusal is tested separately.
Tencent invocation witnesses prove its external transport boundary only; they
are not a TCP witness for TDX. TDX/Eastmoney cases must be invalid requests that
fail in the registered request decoder, not unsupported valid source probes.

## Gates B through D

Only this design, the new test module and its external declaration may change.
Do not lower 95%, alter checker/globs/ignores or move production code. Preserve
source admission, credentials, network policy, all failures and user notes.
Initially passing characterization tests are not production red-green fixes.
A newly discovered defect needs a reproducer, diagnosis and separately justified
minimal repair before production changes.

Run focused, affected-library and full documented checks on one fixed tree.
Review Standards and Spec independently on the exact base and new source, then
publish nonforcibly under existing verified authority. Observe that genuinely
changed HEAD's original CI. Only actual 95% qualification and matching release
controls permit a source/binary/descriptor candidate and real business RPC.
No formal service, daily authority or historical technology query is changed.
