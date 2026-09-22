# LimitPools pre-open route-stop capture — 2026-09-22

This artifact is the raw capture behind the recurring pre-open `LimitPools` outage
recorded in
[`../superpowers/specs/2026-09-22-limit-pool-source-precondition-fallthrough-design.md`](../superpowers/specs/2026-09-22-limit-pool-source-precondition-fallthrough-design.md)
(*The recurring pre-open outage*). It is the whole 09:13–09:27 local window on
2026-09-22, taken against the running production registry, and it is preserved
because the claim it backs is a count of records the server wrote: without the
capture, that count cannot be re-derived after the fact.

The window matters because it does not recur on demand. The first candidate's
guard fires only while the requested trading date is not the source's current
trading date, which happens pre-open and nowhere else in the trading day.

## Capture method

Once every 20 seconds for 14 minutes, in this order:

1. an unpinned `LimitPools` call (`trading_date=2026-09-22`), recorded as
   `win-route-N`;
2. one call pinned to each registered candidate — `win-Eastmoney-N`,
   `win-Tonghuashun-N`, `win-HithinkFinance-N` — recording its code, reason
   codes, `complete` flag and record count;
3. a read of the server's own stderr since the previous read, copied verbatim
   with a `SERVERLOG` marker.

Boundaries as recorded by the capture itself:

```text
2026-09-22T09:13:01.745+08:00 ===== capture start minutes=14 interval=20s trading_date=2026-09-22 =====
2026-09-22T09:13:01.765+08:00 server log size before=1121
2026-09-22T09:27:08.495+08:00 server log size after=9535
2026-09-22T09:27:08.538+08:00 ===== capture end =====
```

The 9535 bytes the server wrote over those 14 minutes hold 36 records: the 35
`provider_route_failure` records transcribed below, and this one, the only other
record the window produced:

```text
2026-09-22T09:27:08.538+08:00 SERVERLOG ts=2026-09-22T01:26:28.3748086Z level=ERROR target=grpc_server event=provider_failure stage=provider_response_invalid request_id="win-HithinkFinance-40" operation=limit_pools provider="HithinkFinance" provider_reason="category=decode"
```

The capture file also holds 83 probe-result lines, which are the capture's own
reading of each call's status and are not server records: 41 for the unpinned
route and 14 for each of the three pinned candidates. They are reproduced in
full under *The capture's per-call readings*, so this artifact is the capture
complete rather than an excerpt of it.

## The 35 unpinned-route failures

`win-route-1` through `win-route-35`, complete and unedited. Every one carries
`attempt_count=1` and `attempts=Eastmoney:source_precondition`.

```text
2026-09-22T09:27:08.513+08:00 SERVERLOG ts=2026-09-22T01:13:03.1638502Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-1" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.514+08:00 SERVERLOG ts=2026-09-22T01:13:24.8492016Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-2" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.514+08:00 SERVERLOG ts=2026-09-22T01:13:44.9602054Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-3" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.515+08:00 SERVERLOG ts=2026-09-22T01:14:05.0609307Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-4" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.515+08:00 SERVERLOG ts=2026-09-22T01:14:26.511666Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-5" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.516+08:00 SERVERLOG ts=2026-09-22T01:14:46.6137693Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-6" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.517+08:00 SERVERLOG ts=2026-09-22T01:15:06.7123762Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-7" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.517+08:00 SERVERLOG ts=2026-09-22T01:15:28.2522156Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-8" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.518+08:00 SERVERLOG ts=2026-09-22T01:15:48.3624196Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-9" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.519+08:00 SERVERLOG ts=2026-09-22T01:16:08.4639095Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-10" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.520+08:00 SERVERLOG ts=2026-09-22T01:16:30.0140228Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-11" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.521+08:00 SERVERLOG ts=2026-09-22T01:16:50.1173846Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-12" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.522+08:00 SERVERLOG ts=2026-09-22T01:17:10.2159884Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-13" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.523+08:00 SERVERLOG ts=2026-09-22T01:17:32.1045104Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-14" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.523+08:00 SERVERLOG ts=2026-09-22T01:17:52.2042962Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-15" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.524+08:00 SERVERLOG ts=2026-09-22T01:18:12.354888Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-16" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.525+08:00 SERVERLOG ts=2026-09-22T01:18:33.9703778Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-17" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.525+08:00 SERVERLOG ts=2026-09-22T01:18:54.0664524Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-18" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.526+08:00 SERVERLOG ts=2026-09-22T01:19:14.1666312Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-19" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.527+08:00 SERVERLOG ts=2026-09-22T01:19:35.6674991Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-20" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.527+08:00 SERVERLOG ts=2026-09-22T01:19:55.7671156Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-21" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.528+08:00 SERVERLOG ts=2026-09-22T01:20:15.8675407Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-22" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.529+08:00 SERVERLOG ts=2026-09-22T01:20:37.3582955Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-23" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.529+08:00 SERVERLOG ts=2026-09-22T01:20:57.4689194Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-24" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.530+08:00 SERVERLOG ts=2026-09-22T01:21:17.5726017Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-25" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.530+08:00 SERVERLOG ts=2026-09-22T01:21:39.1194178Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-26" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.531+08:00 SERVERLOG ts=2026-09-22T01:21:59.2190807Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-27" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.532+08:00 SERVERLOG ts=2026-09-22T01:22:19.3202961Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-28" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.532+08:00 SERVERLOG ts=2026-09-22T01:22:40.8706375Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-29" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.534+08:00 SERVERLOG ts=2026-09-22T01:23:00.9706021Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-30" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.535+08:00 SERVERLOG ts=2026-09-22T01:23:21.0700661Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-31" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.535+08:00 SERVERLOG ts=2026-09-22T01:23:42.5790887Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-32" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.536+08:00 SERVERLOG ts=2026-09-22T01:24:02.6560964Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-33" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.536+08:00 SERVERLOG ts=2026-09-22T01:24:22.7721842Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-34" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
2026-09-22T09:27:08.537+08:00 SERVERLOG ts=2026-09-22T01:24:44.3269089Z level=ERROR target=grpc_server event=provider_route_failure stage=provider_route_stopped request_id="win-route-35" operation=limit_pools attempt_count=1 attempts=Eastmoney:source_precondition
```

The server timestamps run from `01:13:03Z` to `01:24:44Z`, i.e. 09:13:03 to
09:24:44 local — the "35 route-failure lines between 09:13 and 09:24 local" the
design records. **No record names Tonghuashun or HithinkFinance**: with
`attempt_count=1` there was no second attempt, so neither candidate was ever
asked.

## The same window, per candidate

The pinned calls in the same 14 minutes (14 samples each):

| candidate | failed | succeeded |
| --- | --- | --- |
| unpinned route | 35 × `provider_route_stopped`, `attempts=Eastmoney:source_precondition` | 6 × `complete=True` — 09:25:04 with 7 rows, then 5 × 12 rows through 09:26:48 |
| `Eastmoney` | 12 × `FailedPrecondition`, `source_precondition_failed` | 2 × `complete=True`, 12 rows |
| `Tonghuashun` | 12 × `FailedPrecondition`, `source_precondition_failed` | 2 × `complete=True`, 12 rows |
| `HithinkFinance` | 1 × `provider_response_invalid` (`category=decode`) | 13 × `complete=True`, 1 row |

Two things in this table are worth keeping separate from the route-stop defect:

- `Tonghuashun` refused the same date Eastmoney refused, with its own reason
  (`upper-limit pool is empty for 2026-09-22`), and then served it later in the
  window. A candidate refusing pre-open is a fact about that candidate, which is
  what the design's fall-through is built on — it is not evidence that the
  fall-through restores this window.
- `HithinkFinance` answered 13 of 14 samples with `complete=True` and one row,
  and failed one with a decode error. One row is a complete answer for a pool
  that had one member at that moment.

## The capture's per-call readings

The 83 lines the capture wrote for the calls it made, verbatim and in order: 41 for the unpinned route, then 14 for each pinned candidate. These are the capture's own reading of each gRPC status, not server records.

```text
2026-09-22T09:13:03.261+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-1][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:13:04.075+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-1][source_precondition_failed0]
2026-09-22T09:13:04.263+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-1][source_precondition_failed0]
2026-09-22T09:13:04.744+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:13:24.864+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-2][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:13:44.975+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-3][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:14:05.075+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-4][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:14:06.066+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-4][source_precondition_failed0]
2026-09-22T09:14:06.208+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-4][source_precondition_failed0]
2026-09-22T09:14:06.406+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:14:26.525+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-5][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:14:46.628+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-6][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:15:06.726+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-7][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:15:07.720+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-7][source_precondition_failed0]
2026-09-22T09:15:07.908+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-7][source_precondition_failed0]
2026-09-22T09:15:08.159+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:15:28.268+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-8][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:15:48.375+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-9][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:16:08.491+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-10][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:16:09.467+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-10][source_precondition_failed0]
2026-09-22T09:16:09.658+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-10][source_precondition_failed0]
2026-09-22T09:16:09.908+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:16:30.027+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-11][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:16:50.135+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-12][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:17:10.229+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-13][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:17:11.222+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-13][source_precondition_failed0]
2026-09-22T09:17:11.412+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-13][source_precondition_failed0]
2026-09-22T09:17:12.012+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:17:32.119+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-14][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:17:52.219+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-15][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:18:12.368+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-16][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:18:13.321+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-16][source_precondition_failed0]
2026-09-22T09:18:13.716+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-16][source_precondition_failed0]
2026-09-22T09:18:13.861+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:18:33.985+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-17][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:18:54.080+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-18][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:19:14.181+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-19][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:19:15.171+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-19][source_precondition_failed0]
2026-09-22T09:19:15.363+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-19][source_precondition_failed0]
2026-09-22T09:19:15.563+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:19:35.680+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-20][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:19:55.777+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-21][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:20:15.881+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-22][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:20:16.877+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-22][source_precondition_failed0]
2026-09-22T09:20:17.064+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-22][source_precondition_failed0]
2026-09-22T09:20:17.261+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:20:37.374+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-23][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:20:57.484+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-24][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:21:17.585+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-25][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:21:18.615+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-25][source_precondition_failed0]
2026-09-22T09:21:18.820+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-25][source_precondition_failed0]
2026-09-22T09:21:19.016+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:21:39.134+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-26][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:21:59.232+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-27][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:22:19.335+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-28][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:22:20.328+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-28][source_precondition_failed0]
2026-09-22T09:22:20.523+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-28][source_precondition_failed0]
2026-09-22T09:22:20.767+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:22:40.885+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-29][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:23:00.984+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-30][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:23:21.100+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-31][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:23:22.077+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-31][source_precondition_failed0]
2026-09-22T09:23:22.234+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-31][source_precondition_failed0]
2026-09-22T09:23:22.504+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:23:42.592+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-32][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:24:02.672+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-33][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:24:22.783+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-34][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:24:23.781+08:00 exit=73 target=Eastmoney reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: verified empty result: family=limit_pool request_identity=Upper:2026-09-22:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date trailer=[win-Eastmoney-34][source_precondition_failed0]
2026-09-22T09:24:23.970+08:00 exit=73 target=Tonghuashun reason_codes=source_precondition_failed complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-22 trailer=[win-Tonghuashun-34][source_precondition_failed0]
2026-09-22T09:24:24.220+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:24:44.342+08:00 exit=73 target=route reason_codes=provider_route_stopped,rejected,source_precondition complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider route did not produce an admitted batch trailer=[win-route-35][Eastmoney"][provider_route_stopped0][Eastmoney][rejected"][source_precondition]
2026-09-22T09:25:04.422+08:00 exit=0 target=route reason_codes=<none> complete=True records=7 stderr= trailer=
2026-09-22T09:25:24.534+08:00 exit=0 target=route reason_codes=<none> complete=True records=12 stderr= trailer=
2026-09-22T09:25:25.557+08:00 exit=0 target=Eastmoney reason_codes=<none> complete=True records=12 stderr= trailer=
2026-09-22T09:25:26.258+08:00 exit=0 target=Tonghuashun reason_codes=<none> complete=True records=12 stderr= trailer=
2026-09-22T09:25:26.671+08:00 exit=0 target=HithinkFinance reason_codes=<none> complete=True records=1 stderr= trailer=
2026-09-22T09:25:46.786+08:00 exit=0 target=route reason_codes=<none> complete=True records=12 stderr= trailer=
2026-09-22T09:26:06.887+08:00 exit=0 target=route reason_codes=<none> complete=True records=12 stderr= trailer=
2026-09-22T09:26:27.029+08:00 exit=0 target=route reason_codes=<none> complete=True records=12 stderr= trailer=
2026-09-22T09:26:27.987+08:00 exit=0 target=Eastmoney reason_codes=<none> complete=True records=12 stderr= trailer=
2026-09-22T09:26:28.226+08:00 exit=0 target=Tonghuashun reason_codes=<none> complete=True records=12 stderr= trailer=
2026-09-22T09:26:28.389+08:00 exit=73 target=HithinkFinance reason_codes=provider_response_invalid complete=? records=? stderr=ERROR: |   Code: FailedPrecondition |   Message: provider response violated its contract trailer=[win-HithinkFinance-40][HithinkFinance"][provider_response_invalid0]
2026-09-22T09:26:48.489+08:00 exit=0 target=route reason_codes=<none> complete=True records=12 stderr= trailer=
```

## Decision

The capture proves the mechanism the design attributes the outage to: for the
whole window, every unpinned route stopped after exactly one attempt, and that
attempt was Eastmoney's source precondition. It also shows the route healing
inside the same window, without a restart: the last stop is 09:24:44 and the
first success is 09:25:04, one 20-second sample later, once the guard stopped
firing.

That boundary is **earlier** than the 09:30 recovery the design quotes from the
downstream monitor, and this capture does not reconcile the two. The gap is
consistent with the monitor's own call cadence — it reports what its own samples
saw, not the server's first success — but that is an inference, not something
these 36 records show. A window that ends at 09:24:44 on the server's side and at
09:30 on the monitor's is a difference worth resolving before either timestamp is
used as the recovery point.

It does **not** prove that the fall-through change restores the 09:15–09:25
window. Two of the three candidates were themselves refusing that date for part
of the window, so the window may still fail after the change for reasons that
belong to the candidates. The falsifiable re-run is in the design: on the next
trading day, call in the window and read `stage=provider_route_exhausted
attempts=...` — a stopped route now means something different from an exhausted
one.
