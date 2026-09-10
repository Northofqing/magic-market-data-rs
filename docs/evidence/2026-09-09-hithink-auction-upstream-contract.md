# HITHINK 集合竞价上游合同核对 — 2026-09-09

Status: first-party contract review completed; no production admission is
changed by this note.

## Scope and source pin

This note checks how the official `HiThink-Tech/Financial-API` project handles
three gaps in the A-share auction snapshot contract: an absent trading date, a
response-assembly `timestamp`, and one directionless unmatched quantity.

The GitHub source is pinned to official commit
[`44b7aa34dd504675f3ddaa15b3d478ea16f97884`](https://github.com/HiThink-Tech/Financial-API/tree/44b7aa34dd504675f3ddaa15b3d478ea16f97884)
(2026-09-08). The auction REST contract itself has not changed since it was
introduced at
[`9dbef74d2ce535857e610eec265bcb9302942d48`](https://github.com/HiThink-Tech/Financial-API/commit/9dbef74d2ce535857e610eec265bcb9302942d48).
The generated official site presents the same current contract on its
[auction reference page](https://fuyao.aicubes.cn/docs/api-reference/auction/#a股集合竞价快照).

## Verdict

The upstream project does **not** solve these three gaps by reconstructing or
enriching the snapshot. It exposes a deliberately current-only, response-time
snapshot and preserves one undirected `auction_unmatched` value. A separate
auction-benchmark endpoint supplies an explicit date for its own much narrower
result, but the public contract does not bind that response to an auction
snapshot and does not make it a valid source of missing snapshot fields.

| Gap | What the official project actually does | Is the gap solved for the snapshot? |
| --- | --- | --- |
| Missing `trading_date` | `/api/a-share/auction/snapshot` accepts only `thscodes` and `stage`, and returns `{timestamp, auction_phase, data_status, total, item[]}`. Neither the request nor a record carries `date`/`date_ms`. | **No.** The CLI labels its window `today-only`, but that is client metadata, not record-level provider proof. |
| `timestamp` is response assembly time | The REST docs, MCP guidance, Python docstring, and CLI description all explicitly retain that meaning and warn consumers not to treat it as upstream auction time. | **Only the ambiguity is solved.** The project documents the field truthfully; it does not publish a provider event time that can serve as `source_at`. |
| One unmatched quantity has no direction | The response has exactly one nullable `auction_unmatched`; there is no side, sign convention, `unmatched_bid_quantity`, or `unmatched_ask_quantity`. | **No.** Official clients pass the field through and do not infer a side. |

The authoritative snapshot request, top-level response, item fields and
avoidance rules are in the official
[GitHub auction contract, lines 8–42](https://github.com/HiThink-Tech/Financial-API/blob/44b7aa34dd504675f3ddaa15b3d478ea16f97884/docs/api/endpoints-auction.md#L8-L42).
The generated site independently describes `data.timestamp` as response
assembly time and groups volume, amount and one unmatched value on its
[snapshot return-fields section](https://fuyao.aicubes.cn/docs/api-reference/auction/#返回字段).

## Date-related auxiliary endpoints are separate products

The official project offers two date-related endpoints, but neither repairs
the snapshot contract:

- `/api/a-share/auction/short-term-benchmark` accepts an optional
  `date=YYYY-MM-DD` and returns `date`, `date_ms`, `timestamp`, and items with
  only identity, `auction_pct`, and tags. It has no auction price, volume,
  amount, unmatched quantity, phase, or snapshot identity. The official
  contract also says an explicit non-trading date is not rolled back. See
  [GitHub lines 48–68](https://github.com/HiThink-Tech/Financial-API/blob/44b7aa34dd504675f3ddaa15b3d478ea16f97884/docs/api/endpoints-auction.md#L48-L68)
  and the generated
  [benchmark reference](https://fuyao.aicubes.cn/docs/api-reference/auction/#短线风向标竞价基准).
- `/api/a-share/calendar/trading-days` returns a recent trading-day list. It
  can answer whether a Shanghai-calendar date is a trading day, but it carries
  no auction snapshot ID or timestamp relation and therefore cannot prove
  which date a possibly cached snapshot belongs to. See the official
  [calendar contract](https://github.com/HiThink-Tech/Financial-API/blob/44b7aa34dd504675f3ddaa15b3d478ea16f97884/docs/api/endpoints-calendar.md#L1-L39).

Consequently, joining the benchmark or calendar to the snapshot would be a
new downstream assumption across independently timed responses. No such join
or snapshot-date validation exists in the official Python or CLI clients.

## How the official clients and Skill consume it

The Python function validates only `stage` and 1–100 A-share codes, calls the
snapshot endpoint, and returns its `data` object. Its docstring explicitly says
that `data.timestamp` is the response-assembly timestamp. The benchmark is a
separate function whose resolved `date/date_ms` belong to that response. See
[`fuyao_client.py`, lines 655–688](https://github.com/HiThink-Tech/Financial-API/blob/44b7aa34dd504675f3ddaa15b3d478ea16f97884/python/toolkit/fuyao/scripts/fuyao_client.py#L655-L688).
The shared `_get` helper returns `payload.data` without auction enrichment (and
does not preserve the success-envelope `request_id` in the returned value); see
[`fuyao_client.py`, lines 114–146](https://github.com/HiThink-Tech/Financial-API/blob/44b7aa34dd504675f3ddaa15b3d478ea16f97884/python/toolkit/fuyao/scripts/fuyao_client.py#L114-L146).

The Agent Skill routes snapshot and benchmark as separate tools and explicitly
lists treating the snapshot `timestamp` as the upstream auction time as a
common error; see the official
[MCP/Skill guidance, lines 36–41](https://github.com/HiThink-Tech/Financial-API/blob/44b7aa34dd504675f3ddaa15b3d478ea16f97884/skills/hithink-finance/references/mcp/hithink-finance-a-share.md#L36-L41).
Its Python example merely calls
`a_share_auction_snapshot(..., stage="final")` and consumes the raw result; see
the official
[remote-toolkit example, lines 27–48](https://github.com/HiThink-Tech/Financial-API/blob/44b7aa34dd504675f3ddaa15b3d478ea16f97884/skills/hithink-finance/references/python-sdk/remote-toolkit.md#L27-L48).

The Node CLI follows the same approach: snapshot and benchmark are distinct
capabilities, snapshot is marked `today-only`, and the output schema is a
passthrough array of arbitrary records rather than an enriched auction model.
See
[`remote-capabilities.ts`, lines 52–54 and 688–723](https://github.com/HiThink-Tech/Financial-API/blob/44b7aa34dd504675f3ddaa15b3d478ea16f97884/hithink-finance-cli/src/contracts/remote-capabilities.ts#L688-L723).

## Implication for this repository

The official project truthfully exposes a useful **current auction observation**
but does not supply the evidence needed to map it into a contract that requires
an explicit trading date, provider event `source_at`, and directional bid/ask
unmatched quantities. A production integration must therefore either use a
narrow current-observation contract that keeps those fields absent, or obtain a
different first-party response that publishes them. It must not manufacture
them from the local date, the response assembly time, the benchmark, or the
calendar.
