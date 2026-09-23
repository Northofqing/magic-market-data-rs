# Consensus earnings-batch failures attributed to the downstream's local route -- 2026-09-23

This artifact closes the Consensus line that the 2026-09-23 instrument-news handoff left open
(its section 4.1, "Consensus 全线失败, 本轮未诊断"). It records two measurements: that this
server answers Consensus correctly, and that the failing calls in the downstream log are issued
on a route which cannot reach this server at all.

The downstream line under investigation reads:

```text
[21:00:47 WARN] [v17_sources][BR-115] 688690 earnings batch rejected: financials=ok;
  consensus=GrpcBridge data gateway failed reason_code=no_current_reports
  provider=Some(Eastmoney) retryable=false: gRPC Consensus 查询失败: 服务端内部错误
```

Both readings below are preserved rather than summarised: the probe output in full, and the
error-detail trailer as raw bytes.

## 1. This server serves Consensus

`magic.market.consensus.request` version 1, `{"instruments":[{"exchange":"Shanghai","code":"688690","asset_class":"Equity"}]}`,
sent 2026-09-23 13:13:16 UTC (21:13:16 CST) against the deployed build:

```text
Response contents:
{
  "requestId": "claude-consensus-1",
  "operation": "OPERATION_CONSENSUS",
  "admission": "ADMISSION_STATE_ADMITTED",
  "selectedProvider": "Tonghuashun",
  "batchId": "ths:1790169196.202066700:consensus",
  "complete": true,
  "observedAt": "1790169196.202066700",
  "sourceAt": "2026-09-23",
  "records": [
    {
      "schema": "magic.market.consensus_snapshot",
      "schemaVersion": 1,
      "contentType": "application/json; charset=utf-8",
      "data": "eyJpbnN0cnVtZW50Ijp7ImV4Y2hhbmdlIjoiU2hhbmdoYWkiLCJjb2RlIjoiNjg4NjkwIiwiYXNzZXRfY2xhc3MiOiJFcXVpdHkifSwibmFtZSI6Iue6s+W+ruenkeaKgCIsImVzdGltYXRlcyI6W3siZmlzY2FsX3llYXIiOjIwMjYsImVwcyI6MC42MywiZXBzX21pbiI6MC41NSwiZXBzX21heCI6MC43LCJjb250cmlidXRvcl9jb3VudCI6NSwicmV2ZW51ZSI6bnVsbCwicHJvZml0IjpudWxsfSx7ImZpc2NhbF95ZWFyIjoyMDI3LCJlcHMiOjAuODksImVwc19taW4iOjAuNzgsImVwc19tYXgiOjAuOTgsImNvbnRyaWJ1dG9yX2NvdW50Ijo1LCJyZXZlbnVlIjpudWxsLCJwcm9maXQiOm51bGx9LHsiZmlzY2FsX3llYXIiOjIwMjgsImVwcyI6MS4xOCwiZXBzX21pbiI6MS4xMSwiZXBzX21heCI6MS4yMywiY29udHJpYnV0b3JfY291bnQiOjUsInJldmVudWUiOm51bGwsInByb2ZpdCI6bnVsbH1dLCJjb250cmlidXRvcl9jb3VudCI6NSwiZXZpZGVuY2UiOnsicHJvdmlkZXIiOiJUb25naHVhc2h1biIsInNvdXJjZV9hdCI6IjIwMjYtMDktMjMiLCJvYnNlcnZlZF9hdCI6IjE3OTAxNjkxOTYuMjAyMDY2NzAwIiwiYmF0Y2hfaWQiOiJ0aHM6MTc5MDE2OTE5Ni4yMDIwNjY3MDA6Y29uc2Vuc3VzIn19"
    }
  ]
}

Response trailers received:
(empty)
```

Decoded, the single record carries 2026/2027/2028 estimates at 0.63 / 0.89 / 1.18 EPS with
`contributor_count` 5, and the batch is `complete: true` with an empty trailer -- that is, no
`magic-error-detail-bin` and therefore no error to report.

## 2. The failing calls cannot reach this server

The downstream runs two gRPC client profiles. Only one of them targets this VM.

| profile | log label | endpoint | operations |
| --- | --- | --- | --- |
| `ContractProfile::LocalBridgeV1` | `GrpcBridge` | `GRPC_MARKET_ADDR`, default `http://127.0.0.1:18082` | Consensus (`market.consensus` v1), T0Evidence, OrderBooks |
| `ContractProfile::ExternalV1` | `GrpcExternalV1` | `10.211.55.3:50051`, mTLS + Bearer + authority `magic-market.local` | SecurityMetadata, GlobalNews, InstrumentNews only |

Consensus is issued on the first, not the second:

- `src/data_gateway/grpc_source.rs:3514` `consensus_async` sends `{"codes": [code]}` through
  `query_op` on `ContractProfile::LocalBridgeV1`.
- `src/grpc_client/client.rs:1045` selects the endpoint by profile; `grpc_source.rs:70` holds
  `DEFAULT_ADDR = "http://127.0.0.1:18082"`.
- `src/grpc_client/external_v1.rs` declares no Consensus operation at all.

`127.0.0.1:18082` is the downstream machine's own locally maintained `grpc_market_server`, built
2026-08-30 and left running when the in-repository provider host was removed from their tree; it
emits its errors on `grpc-status-details-bin`, not on the `magic-error-detail-bin` this server
writes. Their own `2026-09-23-tdx-historical-bars-local-route-investigation.md` reached the same
locality for a different failing operation.

## 3. The reverse measurement: their envelope, this server

Feeding this server the exact LocalBridgeV1 envelope (schema `market.consensus` v1, base64 data
`{"codes":["688690"]}`) is not a route that exists here, and the server says so rather than
failing internally:

```text
Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Wed, 23 Sep 2026 14:10:49 GMT
magic-error-detail-bin: ChhjbGF1ZGUtY29uc2Vuc3VzLWxvY2FsLTEQICIPaW52YWxpZF9yZXF1ZXN0MAI=
Sent 1 request and received 0 responses
ERROR:
  Code: InvalidArgument
  Message: consensus requires schema magic.market.consensus.request version 1
```

The trailer decodes field by field as:

| bytes | field | value |
| --- | --- | --- |
| `0A 18` | 1, request_id | `claude-consensus-local-1` |
| `10 20` | 2, operation | 32 (`Operation::Consensus`) |
| `22 0F` | 4, reason_code | `invalid_request` |
| `30 02` | 6, admission | 2 (`ADMISSION_STATE_UNADMITTED`) |

`retryable` (field 5) is absent, so it reads as false. Recorded as-is: this decode path reports
the request as unadmitted even though the same operation returns `ADMISSION_STATE_ADMITTED` for
the correct schema, and the downstream's `map_query_error` classifies `InvalidArgument` from the
status code before it ever looks at the trailer.

The consequence for the investigation is decisive: **rerouting `18082` at this server would turn
those lines into `InvalidArgument`/`invalid_request`, never into `Internal`.** The 211 `internal`
readings in the log cannot have originated here.

## 4. Where `no_current_reports` comes from

The string appears nowhere in this repository: 479 commits reachable from all refs, no match, and
`git log --all -S 'no_current_reports'` is empty. It is produced by the downstream's own deleted
provider host, at `120b90dc:src/data_gateway/consensus.rs`, whose Consensus is Eastmoney research
reports inside a 180-day window:

```rust
const REPORT_WINDOW_DAYS: i64 = 180;
...
let begin = today - Duration::days(REPORT_WINDOW_DAYS);
if admitted.is_empty() {
    return Err(GatewayError::classified(
        CAPABILITY, Some(ProviderId::Eastmoney), "unavailable",
        "no_current_reports", false,
        format!("typed provider returned no reports in admitted window {begin}..={today}"),
    ));
}
```

The provider, the code and the non-retryable flag match the logged line verbatim. The sibling
`GatewayError::invalid_evidence(...)` in the same file accounts for the `invalid_evidence`
readings.

## 5. Count decomposition

30 MB tail of `monitor-launchd-20260916.stderr.log`, lines carrying both `consensus=` and
`reason_code=`: 1647 lines, 15:03:51 to 21:10:40 CST, 27 distinct instruments.

| reading | count |
| --- | --- |
| `reason_code=no_current_reports` | 1285 |
| `reason_code=internal` | 211 |
| `reason_code=invalid_evidence` | 136 |
| `reason_code=no_verified_batch` | 31 |
| `provider=Some(Eastmoney)` | 1632 |
| `provider=None` | 31 |

A line can carry more than one reason code, so the codes do not sum to the line count. Two
readings are explained by the client rather than by any server: `provider=None` (31) coincides
exactly with `no_verified_batch` (31) and is the client's `unwrap_or` default when it decoded no
trailer at all, and `internal` (211) is `reason_code_static` folding a wire value that is not in
its known list. This server's closed set is seven codes -- `capability_unadmitted`,
`source_precondition_failed`, `invalid_evidence`, `invalid_request`, `internal`,
`provider_route_exhausted`, `provider_route_stopped` -- and `no_current_reports` is not among
them.

## 6. What this artifact does not establish

- It does not establish the state of the downstream's `18082` service at any time; no call was
  made from that machine.
- It does not establish that the 27 instruments have no research reports anywhere, only that the
  deleted provider host's 180-day Eastmoney window found none.
- It does not establish the origin of the 211 `internal` readings beyond ruling this server out;
  it establishes only that they are not a member of this server's closed set, and that this
  server's answer to that envelope is `InvalidArgument`.

## 7. Reproduce

Probe scripts are scratch under `target\runtime\probe\` and are not deliverables.

```powershell
# this server, correct contract
target\runtime\probe\call-consensus.ps1 -Codes 688690 -Out target\runtime\probe\consensus-688690.out.txt
# this server, the downstream's LocalBridgeV1 envelope
Copy-Item target\runtime\probe\req-consensus-local.json target\runtime\probe\req-consensus.json
cmd /c target\runtime\probe\call-consensus.cmd   # -> InvalidArgument, trailer above
# the downstream log (148 MB, seek-based tail, not Get-Content)
target\runtime\probe\consensus-scan.ps1 -Megabytes 30
```

## 8. Handoff state

Closed: the Consensus failure line. It is the downstream's local route, and the reason code is
theirs.

Carried forward, unchanged from the previous handoff and not addressed here:

- The supply ceiling on Sina instrument news is a rule requirement, not a defect: at limit >= 199
  five pages cannot prove completeness, so BR-025 requires an explicit failure. Recommended
  downstream limit is <= 195; their current limit of 100 is unaffected.
- Widening BR-025's five-page bound is an untaken option. It is a business-rule change and would
  need a Gate A design plus provider admission evidence. Not attempted.
- The standing request that downstream failure records carry instrument and limit still stands;
  this investigation had to reconstruct both from log text.

Two options were put to the downstream and neither is decided here, because both change a public
contract or a product behaviour:

- A: move Consensus onto ExternalV1. This requires a new `magic.market.consensus.*` external
  operation here, plus a field comparison between this server's Tonghuashun consensus and their
  `ConsensusData` (their index GD-004 already records that the conversion drops the most recent
  report, dates and target prices).
- B: leave Consensus on their local provider host, in which case the semantics of
  `no_current_reports` and the BR-115 earnings-batch rejection policy need to be aligned by that
  host's maintainer.
