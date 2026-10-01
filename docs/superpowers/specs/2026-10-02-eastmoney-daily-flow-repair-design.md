# Eastmoney daily fund flow bounded repair design

Status: the Mac coordinating task explicitly accepted this exact Gate A scope and the public FundFlowSeries test seam in its 2026-10-02 feedback under the existing human-authorized repair task. Implementation may proceed within that scope only. This confirmation grants no new deployment, network, credential, TLS, or execution-policy authority. Live new-path admission and same-build gRPC acceptance remain pending.

## Evidence and diagnostic boundary

Production `MoneyFlows` normalizes one instrument and invokes the public `FundFlowSeries::fund_flow_series` trait with Day1 and limit 1. The inspected current provider selects `push2delay.eastmoney.com/api/qt/stock/fflow/kline/get` for Day1. The first-party stock page now constructs `push2his.eastmoney.com/api/qt/stock/fflow/daykline/get`, public page parameter ut, and cb JSONP. See [primary-source controls](../../integrations/research/2026-10-02-eastmoney-native-flow-request-controls.md) and [provider source](../../../crates/magic-eastmoney-rs/src/fund_flow.rs).

One retained native response is positive for SZ 300005 on September 29/30. Two further credential-free curl controls at 2026-10-01 19:17 UTC, with limits 1 and 2, both returned HTTP 000, zero body bytes and curl 56 missing close_notify. Their raw response headers/stderr and command-derived summary are retained at `target/eastmoney-native-repeat-20261002-02280636a0f14fefaaa6b06695d25ef2`. Their query ordering differs from the original documented successful URI. Two additional controls preserved the exact original URI and query order with limit 2, and also failed before HTTP status; their originals are in `target/eastmoney-native-request-repeat-20261002`. The original command specified no custom UA, Referer or Origin, but its actual request headers were not captured and cannot be recovered from later repeats. See the [repeat addendum](../../integrations/research/2026-10-02-eastmoney-native-flow-repeat-controls.md). These are native diagnostics, not normalized Provider admission or gRPC receipts. Do not claim that the new path, ut or JSONP alone fixes the transport failure.

## Proposed implementation scope

Only the private Day1 request and response handling would change. Select the current first-party HTTPS daykline path on the already-allowlisted push2his host; preserve the caller's existing positive Core row bound (1 through 10000) as lmt instead of the webpage's unbounded lmt=0. Add the public first-party ut value and fixed repository-owned ASCII cb value `emProbe`, matching the retained native sample; the literal must be recorded in tests and live requests, not supplied by an external caller. A repeatable new-path contract is not yet established for limit 1 or 2, let alone the full accepted limit domain. Minute1 keeps its existing endpoint, parameters and JSON parsing unchanged.

Decode JSONP as data only: require exact fixed callback prefix and exact closing delimiter, permit only bounded outside ASCII whitespace, parse only the enclosed JSON object with serde_json and reject wrong callbacks, comments, appended statements, multiple invocations, malformed/non-object JSON and invalid UTF-8. Never execute JavaScript, eval, use an arbitrary callback regex, strip to the first brace or accept a script's partial JSON. Existing rc, source market/code, calendar date, numbers, record/batch evidence, source time and empty-result validation remain required. A transport error is not an empty response.

No HTTP/TLS dependency, endpoint allowlist, MIME policy, timeout, body cap, redirect policy, pacing, retry policy, Core record type, protobuf contract or provider identity changes are proposed. Minute1 and BoardFlows are explicitly out of implementation scope. BoardFlows still lacks a successful native response and its native field/filter differences cannot be promoted into a verified fix.

## Confirmed test seam

The public boundary under test is the public Provider entry `EastmoneyClient` implementing `FundFlowSeries::fund_flow_series`, which is what the production MoneyFlows handler calls. Replace only the external HTTP response at the existing `EastmoneyTransport` boundary. This does not start a gRPC listener or bypass the isolated-service launch refusal.

The coordinating task confirmed this boundary, fixed emProbe callback, positive bounded lmt mapping, and unchanged Minute1/Board/Core/proto/TLS/dependency/pacing scope. Take one vertical slice at a time:

1. Replay the preserved original native JSONP bytes through the public trait with SZ 300005, Day1, limit 2. Assert the independently captured dates and exact CNY net-flow values, the bounded native request, and unchanged provider/record/batch provenance. Run it red on the old implementation before changing it.
2. Implement only enough Day1 routing and strict data-only parsing for that test to pass.
3. Add public-interface rejection cases for wrong callback, appended script, malformed JSON, source identity mismatch and native rc failures. Preserve the Minute1 regression and unsupported interval/board behavior.
4. Run focused provider/composition tests, formatting, relevant all-target Clippy, compliance and docs checks. Keep actual red and green logs; do not call deterministic tests live admission.
5. Collect repeated bounded normalized Rust Provider positives with exact identity/time/units/evidence and actual pacing. Preserve every failure; do not advance admissions.tsv counts for curl-only controls or imply Minute1 was revalidated.
6. Rebuild the precise final source, then obtain actual same-build Health/Capabilities and business RPC receipts only through a normally authorized isolated acceptance path. Do not deploy before review and exact public tuple feedback to Mac.

This proposal follows [BR-009/010/021/045](../../business_rules.md) and [Gate A through D](../../ENGINEERING_RULES.md). A changing public route needs its provenance/admission notes updated with actual scope and evidence, not a timeless success claim.

## Existing isolated startup refusal

The earlier rejected composite PowerShell action checked loopback 50056, started the candidate with hidden Start-Process, planned same-build announcement v1/v2 read-only probes, then cleanup of only its own started PID and temporary variable. The execution tool refused before command process creation with `rejected: blocked by policy`; no precise matched rule was returned. No candidate Health receipt exists. A different launcher or unapproved production replacement is not an acceptable workaround. The Windows environment must supply a normal policy-permitted acceptance path before that stage.
