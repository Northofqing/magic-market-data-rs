# LimitPools pre-open route fall-through capture -- 2026-09-23

This artifact is the whole 09:13-09:27 local window on 2026-09-23, taken against the
deployed server, and it is the falsifiable re-run the
[2026-09-22 capture](2026-09-22-limit-pools-window-capture.md) asked for: read the
route stage on the next trading day and see whether a stopped route has become an
exhausted one -- or, as here, whether the route stops being recorded at all.

The preceding capture recorded 35 unpinned-route failures, every one of them
`provider_route_stopped attempt_count=1 attempts=Eastmoney:source_precondition`: the
route gave up after the first candidate, whose source precondition refuses the
requested trading date while the source is still on the previous one. The window
matters because it does not recur on demand -- that guard fires pre-open and nowhere
else in the trading day.

The capture is preserved rather than summarised because the claim it backs is a
count of records the server wrote, and that count cannot be re-derived after the fact.

## Capture method

Once every 20 seconds for 14 minutes, in this order:

1. an unpinned `LimitPools` call (`trading_date=2026-09-23`), recorded as `win-route-N`;
2. one call pinned to each registered candidate -- `win-Eastmoney-N`, `win-Tonghuashun-N`,
   `win-HithinkFinance-N` -- recording its code, reason codes, `complete` flag and record
   count;
3. a read of the server's own stderr since the previous read, copied verbatim with a
   `SERVERLOG` marker.

Boundaries as recorded by the capture itself:

```text
2026-09-23T09:13:03.412+08:00 ===== capture start minutes=14 interval=20s trading_date=2026-09-23 kind=Upper limit=50 =====
2026-09-23T09:13:03.855+08:00 server pid=49960 started=2026-09-22T23:51:27+08:00 sha256=7EF4CA4A0696E7B93DEF4A91385ED085429847F8E4115BDE4AC33AEA9419F948
2026-09-23T09:13:03.895+08:00 server log size before=6059 file=C:\DevelopFile\magic-market-data-rs\target\runtime\logs\grpc-server.stderr.log
2026-09-23T09:27:26.709+08:00 server log size after=13948
2026-09-23T09:27:26.713+08:00 ===== capture end =====
```

The capture wrote 134 lines: 84 per-call readings and
45 server records.

## The route calls

All 42 unpinned-route calls, complete and unedited.

```text
2026-09-23T09:13:04.700+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790125967690
2026-09-23T09:13:25.961+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790125997709
2026-09-23T09:13:46.199+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126017924
2026-09-23T09:14:06.458+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126028899
2026-09-23T09:14:27.988+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126048464
2026-09-23T09:14:48.220+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126059533
2026-09-23T09:15:08.422+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126078511
2026-09-23T09:15:29.902+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126120123
2026-09-23T09:15:50.148+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126138988
2026-09-23T09:16:10.355+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126150213
2026-09-23T09:16:32.186+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126180585
2026-09-23T09:16:52.388+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126211112
2026-09-23T09:17:12.636+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126211112
2026-09-23T09:17:33.979+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126230623
2026-09-23T09:17:54.157+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126272621
2026-09-23T09:18:14.403+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126272621
2026-09-23T09:18:35.826+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126302672
2026-09-23T09:18:56.078+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126332693
2026-09-23T09:19:16.284+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126352021
2026-09-23T09:19:37.765+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126352021
2026-09-23T09:19:57.963+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126393116
2026-09-23T09:20:18.192+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126412226
2026-09-23T09:20:39.618+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126423269
2026-09-23T09:20:59.820+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126442265
2026-09-23T09:21:20.021+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126453423
2026-09-23T09:21:41.495+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126472454
2026-09-23T09:22:01.817+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126513630
2026-09-23T09:22:22.039+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126532663
2026-09-23T09:22:43.921+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126562734
2026-09-23T09:23:04.148+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126573851
2026-09-23T09:23:24.341+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126603956
2026-09-23T09:23:45.888+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126623032
2026-09-23T09:24:06.214+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126633993
2026-09-23T09:24:26.458+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126653074
2026-09-23T09:24:47.889+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126683217
2026-09-23T09:25:08.033+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:25:28.162+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:25:49.711+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:26:09.846+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:26:29.979+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:26:51.472+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:27:11.691+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
```

Not one of them is a failure:

| reading | count |
| --- | --- |
| `exit=0`, `records=0` | 35 |
| `exit=0`, records>0 | 7 |
| failure | 0 |

The empty ones run from `2026-09-23T09:13:04.700+08:00` (selected=HithinkFinance) to `2026-09-23T09:24:47.889+08:00`.
The non-empty ones begin at `2026-09-23T09:25:08.033+08:00` (selected=Eastmoney, records=4).

## What the server recorded about the route

| item | count |
| --- | --- |
| server records in the window | 45 |
| `request_id=win-route-*` records | 0 |
| `stage=provider_route_stopped` | 0 |
| `stage=provider_route_exhausted` | 0 |
| `event=provider_failure` | 1 |
| `event=service_failure` | 44 |

Records by `operation`:

| operation | count |
| --- | --- |
| `global_news` | 20 |
| `limit_pools` | 25 |

Every server record, verbatim and unedited:

```text
ts=2026-09-23T01:13:05.1927359Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-1" operation=limit_pools
ts=2026-09-23T01:13:05.4842118Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-1" operation=limit_pools
ts=2026-09-23T01:13:14.2145331Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790125993935-74628-1194" operation=global_news
ts=2026-09-23T01:13:44.192762Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126023926-74628-1224" operation=global_news
ts=2026-09-23T01:14:07.3656819Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-4" operation=limit_pools
ts=2026-09-23T01:14:07.5484514Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-4" operation=limit_pools
ts=2026-09-23T01:14:14.2001443Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126053927-74628-1255" operation=global_news
ts=2026-09-23T01:14:44.2020732Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126083940-74628-1283" operation=global_news
ts=2026-09-23T01:15:09.3353405Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-7" operation=limit_pools
ts=2026-09-23T01:15:09.4783647Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-7" operation=limit_pools
ts=2026-09-23T01:15:14.2857562Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126113999-74628-1307" operation=global_news
ts=2026-09-23T01:15:44.2050807Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126143898-74628-1332" operation=global_news
ts=2026-09-23T01:16:11.2701925Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-10" operation=limit_pools
ts=2026-09-23T01:16:11.3978794Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-10" operation=limit_pools
ts=2026-09-23T01:16:14.1983808Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126173893-74628-1358" operation=global_news
ts=2026-09-23T01:16:44.2040647Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126203895-74628-1380" operation=global_news
ts=2026-09-23T01:16:54.9129426Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126214633-74628-1395" operation=global_news
ts=2026-09-23T01:17:13.5108052Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-13" operation=limit_pools
ts=2026-09-23T01:17:13.6503656Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-13" operation=limit_pools
ts=2026-09-23T01:17:14.2155323Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126233935-74628-1424" operation=global_news
ts=2026-09-23T01:17:44.1994205Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126263895-74628-1455" operation=global_news
ts=2026-09-23T01:18:14.2132948Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126293905-74628-1488" operation=global_news
ts=2026-09-23T01:18:15.2886382Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-16" operation=limit_pools
ts=2026-09-23T01:18:15.4275776Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-16" operation=limit_pools
ts=2026-09-23T01:18:44.1916116Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126323891-74628-1513" operation=global_news
ts=2026-09-23T01:19:14.2024056Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126353907-74628-1539" operation=global_news
ts=2026-09-23T01:19:17.2037985Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-19" operation=limit_pools
ts=2026-09-23T01:19:17.3501486Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-19" operation=limit_pools
ts=2026-09-23T01:19:44.2014376Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126383898-74628-1570" operation=global_news
ts=2026-09-23T01:20:14.2072768Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126413906-74628-1606" operation=global_news
ts=2026-09-23T01:20:19.0822068Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-22" operation=limit_pools
ts=2026-09-23T01:20:19.2476831Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-22" operation=limit_pools
ts=2026-09-23T01:20:44.212898Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126443905-74628-1634" operation=global_news
ts=2026-09-23T01:21:14.2583321Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126473910-74628-1670" operation=global_news
ts=2026-09-23T01:21:20.9448415Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-25" operation=limit_pools
ts=2026-09-23T01:21:21.087056Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-25" operation=limit_pools
ts=2026-09-23T01:21:44.2184003Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126503908-74628-1700" operation=global_news
ts=2026-09-23T01:22:14.2559249Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="1790126533954-74628-1740" operation=global_news
ts=2026-09-23T01:22:22.955138Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-28" operation=limit_pools
ts=2026-09-23T01:22:23.1887853Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-28" operation=limit_pools
ts=2026-09-23T01:23:25.2618458Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-31" operation=limit_pools
ts=2026-09-23T01:23:25.3881219Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-31" operation=limit_pools
ts=2026-09-23T01:24:27.3238638Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-34" operation=limit_pools
ts=2026-09-23T01:24:27.4546482Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Tonghuashun-34" operation=limit_pools
ts=2026-09-23T01:26:31.3189461Z level=ERROR target=grpc_server event=provider_failure stage=provider_response_invalid request_id="win-HithinkFinance-40" operation=limit_pools provider="HithinkFinance" provider_reason="category=decode"
```

## The same window, per candidate

The pinned calls in the same 14 minutes:

| candidate | calls | failed | succeeded | rows on success | first success |
| --- | --- | --- | --- | --- | --- |
| unpinned route | 42 | 0 | 42 | 7 x 4 rows | 2026-09-23T09:25:08.033+08:00 |
| `Eastmoney` | 14 | 12 | 2 | 2 x 4 rows | 2026-09-23T09:25:29.169+08:00 |
| `Tonghuashun` | 14 | 12 | 2 | 2 x 4 rows | 2026-09-23T09:25:29.462+08:00 |
| `HithinkFinance` | 14 | 1 | 13 | 13 x 0 rows | 2026-09-23T09:13:05.658+08:00 |

## The capture's per-call readings

The 84 lines the capture wrote for the calls it made, verbatim and in
order. These are the capture's own reading of each gRPC status, not server records.

```text
2026-09-23T09:13:04.700+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790125967690
2026-09-23T09:13:05.205+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:13:05.504+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:13:05.658+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790125967690
2026-09-23T09:13:25.961+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790125997709
2026-09-23T09:13:46.199+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126017924
2026-09-23T09:14:06.458+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126028899
2026-09-23T09:14:07.387+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:14:07.571+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:14:07.746+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126028899
2026-09-23T09:14:27.988+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126048464
2026-09-23T09:14:48.220+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126059533
2026-09-23T09:15:08.422+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126078511
2026-09-23T09:15:09.352+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:15:09.489+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:15:09.663+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126089711
2026-09-23T09:15:29.902+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126120123
2026-09-23T09:15:50.148+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126138988
2026-09-23T09:16:10.355+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126150213
2026-09-23T09:16:11.284+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:16:11.412+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:16:11.595+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126150213
2026-09-23T09:16:32.186+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126180585
2026-09-23T09:16:52.388+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126211112
2026-09-23T09:17:12.636+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126211112
2026-09-23T09:17:13.528+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:17:13.664+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:17:13.786+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126230623
2026-09-23T09:17:33.979+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126230623
2026-09-23T09:17:54.157+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126272621
2026-09-23T09:18:14.403+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126272621
2026-09-23T09:18:15.303+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:18:15.445+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:18:15.624+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126291908
2026-09-23T09:18:35.826+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126302672
2026-09-23T09:18:56.078+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126332693
2026-09-23T09:19:16.284+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126352021
2026-09-23T09:19:17.218+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:19:17.365+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:19:17.538+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126332693
2026-09-23T09:19:37.765+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126352021
2026-09-23T09:19:57.963+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126393116
2026-09-23T09:20:18.192+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126412226
2026-09-23T09:20:19.097+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:20:19.263+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:20:19.415+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126393116
2026-09-23T09:20:39.618+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126423269
2026-09-23T09:20:59.820+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126442265
2026-09-23T09:21:20.021+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126453423
2026-09-23T09:21:20.955+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:21:21.100+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:21:21.276+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126453423
2026-09-23T09:21:41.495+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126472454
2026-09-23T09:22:01.817+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126513630
2026-09-23T09:22:22.039+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126532663
2026-09-23T09:22:22.976+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:22:23.208+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:22:23.707+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126513630
2026-09-23T09:22:43.921+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126562734
2026-09-23T09:23:04.148+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126573851
2026-09-23T09:23:24.341+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126603956
2026-09-23T09:23:25.276+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:23:25.401+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:23:25.675+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126592873
2026-09-23T09:23:45.888+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126623032
2026-09-23T09:24:06.214+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126633993
2026-09-23T09:24:26.458+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126653074
2026-09-23T09:24:27.338+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=verified empty result: family=limit_pool request_identity=Upper:2026-09-23:limit=50 reason=source returned data.tc=0 and data.pool=[] for the exact trading date
2026-09-23T09:24:27.467+08:00 exit=73 target=Tonghuashun code=FailedPrecondition complete=? records=? message=Tonghuashun response is incomplete: upper-limit pool is empty for 2026-09-23
2026-09-23T09:24:27.605+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126664038
2026-09-23T09:24:47.889+08:00 exit=0 target=route selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126683217
2026-09-23T09:25:08.033+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:25:28.162+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:25:29.169+08:00 exit=0 target=Eastmoney selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:25:29.462+08:00 exit=0 target=Tonghuashun selected=Tonghuashun complete=true records=4 source_at=2026-09-23
2026-09-23T09:25:29.586+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=0 source_at=unix-ms:1790126724389
2026-09-23T09:25:49.711+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:26:09.846+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:26:29.979+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:26:30.981+08:00 exit=0 target=Eastmoney selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:26:31.153+08:00 exit=0 target=Tonghuashun selected=Tonghuashun complete=true records=4 source_at=2026-09-23
2026-09-23T09:26:31.338+08:00 exit=73 target=HithinkFinance code=FailedPrecondition complete=? records=? message=provider response violated its contract
2026-09-23T09:26:51.472+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
2026-09-23T09:27:11.691+08:00 exit=0 target=route selected=Eastmoney complete=true records=4 source_at=2026-09-23
```

## Decision

The route no longer stops at the first candidate. Across the whole window not one
unpinned call failed, and not one `provider_route_stopped` record was written -- where
the preceding capture wrote 35 of them, this one writes none. The route now reaches a
candidate that will serve the requested date and returns OK.

The selection flips at the boundary the guard predicts, and it flips in the right
direction: while Eastmoney is refusing the date the route lands on HithinkFinance with
no rows, and from the first sample after the guard stops firing the route takes
Eastmoney -- its own first preference -- with rows. That is the fall-through doing what
the design intends, and then getting out of the way.

One more reading this window carries, which the fix is not about. The candidates do not
agree on the pool: `HithinkFinance` answered 13 of its 14 pinned
samples with `complete=true` and 0 rows, and that includes the samples taken after the
boundary, in the same window where `Eastmoney` and `Tonghuashun` each returned
4 rows. The preceding capture shows the same shape -- `HithinkFinance`
reporting 1 row where the other two reported 12.

So the route's pre-open answer is `HithinkFinance`'s empty one, and it is returned as a
success. That changes what a pre-open caller observes: the pre-fix binary answered
`FailedPrecondition`, and the fixed one answers OK with `complete=true` and no records. It
follows from the fall-through rather than being a defect in it. This capture does not
establish which candidate holds the right pool -- only that they differ, and which of them
the route presently reports.

It does **not** prove the pool contents are right. `records` is a count, not a
verification, and this capture does not compare any row against the source.

The capture also cannot show an `attempts=` list, because a route that succeeds
writes no record at all. The fall-through evidence here is the selected provider on
each call, not an attempt list: a `selected=HithinkFinance` while Eastmoney is
refusing is the route having asked Eastmoney and moved on, and the pinned calls in
the same tick show Eastmoney refusing. A reader looking for the `attempts=` line the
preceding capture is built on will not find one in this window, and its absence is
the success, not a gap in the capture.

