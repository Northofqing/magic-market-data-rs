# LimitPools route fall-through, measured on the deployed build — 2026-09-22

[`2026-09-22-limit-pool-source-precondition-fallthrough-design.md`](../superpowers/specs/2026-09-22-limit-pool-source-precondition-fallthrough-design.md)
predicts a client-visible outcome: a source precondition from the first candidate
advances the route instead of stopping it, and a later candidate that can attest the
date answers. That prediction was written against the running registry, which at the
time still carried the pre-fix binary — the design's *Evidence* table records the
**failure**, not the fix.

This artifact is the measurement of the same code path against the deployed
**post-fix** binary, taken after the rebuilt server was started. It is committed
because the difference between a prediction and a reading is the whole of Gate D.

## The binary that answered

```
server pid=49960 started=2026-09-22T23:51:27+08:00 sha256=7EF4CA4A0696E7B93DEF4A91385ED085429847F8E4115BDE4AC33AEA9419F948
```

The same digest is the one `cargo build --release -p magic-market-grpc-server`
produced in this working tree, and the running process reports it as its own
executable. The server that answered the request below is therefore the fixed build
and not the one the design's earlier reading was taken against.

## The reading

One round of the probe — an unpinned route call and one call pinned to each
registered candidate — for `trading_date=2026-09-21`, a past trading date that
exercises the same `qdate` guard the pre-open window does. Verbatim:

```text
2026-09-22T23:53:09.918+08:00 ===== capture start minutes=14 interval=20s trading_date=2026-09-21 kind=Upper limit=50 =====
2026-09-22T23:53:10.156+08:00 server pid=49960 started=2026-09-22T23:51:27+08:00 sha256=7EF4CA4A0696E7B93DEF4A91385ED085429847F8E4115BDE4AC33AEA9419F948
2026-09-22T23:53:10.170+08:00 server log size before=939 file=C:\DevelopFile\magic-market-data-rs\target\runtime\logs\grpc-server.stderr.log
2026-09-22T23:53:10.493+08:00 exit=0 target=route selected=Tonghuashun complete=true records=50 source_at=2026-09-21
2026-09-22T23:53:11.331+08:00 exit=73 target=Eastmoney code=FailedPrecondition complete=? records=? message=Eastmoney protocol error: limit-pool source qdate 2026-09-22 does not match requested date 2026-09-21
2026-09-22T23:53:11.520+08:00 exit=0 target=Tonghuashun selected=Tonghuashun complete=true records=50 source_at=2026-09-21
2026-09-22T23:53:11.710+08:00 exit=0 target=HithinkFinance selected=HithinkFinance complete=true records=50 source_at=unix-ms:1790092391547
2026-09-22T23:53:11.814+08:00 SERVERLOG ts=2026-09-22T15:53:11.3135246Z level=ERROR target=grpc_server event=service_failure stage=source_precondition_failed request_id="win-Eastmoney-1" operation=limit_pools
2026-09-22T23:53:11.840+08:00 server log size after=1108
2026-09-22T23:53:11.847+08:00 ===== capture end =====
```

The capture is internally complete: it contains both halves of the claim at once.

- **The precondition still fires.** The pinned `Eastmoney` call returns
  `FailedPrecondition` with the qdate message verbatim, and the server writes the
  bounded `service_failure stage=source_precondition_failed` record for it. The
  guard itself is untouched, exactly as the design's *Decision 3* requires.
- **The route no longer stops on it.** The unpinned call — the same provider order,
  the same date — returns `exit=0` with `selected=Tonghuashun`, `complete=true` and
  50 records provenanced `source_at=2026-09-21`. It reached a candidate that could
  attest the date.

The window produced **no `provider_route_failure` record at all**, because the
unpinned route succeeded. The pre-fix binary wrote one for every such call: the
2026-09-22 window capture holds 35 of them, every one `attempt_count=1
attempts=Eastmoney:source_precondition`.

## Decision

**It proves** the fix is live in the deployed binary and behaves as designed on a
live request: a candidate-scoped source precondition advances the route to a
candidate that can attest the date, while the candidate that raised it is still
recorded as a bounded attempt and the date guard itself is unchanged.

**It does not prove** the 09:15–09:25 pre-open window is restored. That window only
exists pre-open, and this reading was taken after the close, so the past-date
divergence stands in for a guard that is provably the same code path but is not the
same clock. This is exactly the limit the design states for itself, and a probe is
scheduled for the next trading day's 09:13–09:27 window to test it directly.

**It is not a same-request pair with the design's pre-fix reading.** That reading was
taken at `limit=10`; this one at `limit=50`. The claim under test — whether a source
precondition stops the route — does not depend on the limit, and the capture above
establishes both halves of it without needing the pre-fix half: the precondition is
observed firing in the same capture in which the route is observed not stopping.
