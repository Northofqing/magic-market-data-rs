# FuturesDelivery planned calendar, wire contract v2

The active request and record schema version is 2. Version 1 requests are rejected rather than
silently receiving a response with changed meaning.

The admitted provider is `Cffex`. Supply `QueryRequest.context.protocol_version=1`,
`operation=OPERATION_FUTURES_DELIVERY` through the FuturesDelivery RPC,
`preferred_provider="Cffex"`, and `allow_unadmitted=false`. The payload is a
`CanonicalPayload` with `schema="magic.market.futures_delivery.request"`,
`schema_version=2`, `content_type="application/json; charset=utf-8"`, and
`data=base64(UTF-8 JSON {"year":2026,"month":9})`. Both fields are required;
`month` is 1–12. Only 2026 is admitted. A request for 2027 does not mean a
verified empty month: it fails with `UNIMPLEMENTED` and an explicit unsupported
year message. Missing/invalid fields fail with `INVALID_ARGUMENT`.

For each admitted month, the response has `admission=ADMISSION_STATE_ADMITTED`,
`selectedProvider="Cffex"`, `complete=true`, a revisioned `batchId`, and exactly
four `records` (IF, IH, IC, IM). `complete` means that the four **planned**
schedule rows are present; it does **not** mean delivery occurred or that an
exchange adjustment can no longer happen. `sourceAt` is absent because no
single monthly source publication timestamp is claimed. `observedAt` and each
record's `evidence.observed_at` mark local response construction, not source
publication. Each record is a `CanonicalPayload` with
`schema="magic.market.futures_delivery_event"`, `schema_version=2`, and base64
UTF-8 JSON data. See the [full 2026-09 wire fixture](futures-delivery-2026-09.fixture.json).
That fixture is a synthetic, credential-free v2 success response with a fixed
observation timestamp; it is not a claim that this exact request was served.

Decoded record JSON has precisely these fields:

```json
{
  "product": "If",
  "contract_code": "IF2609",
  "last_trading_date": "2026-09-18",
  "delivery_date": "2026-09-18",
  "method": "Cash",
  "schedule_status": "Planned",
  "date_basis": "CffexRuleAndPublishedHolidays",
  "rule_url": "https://www.cffex.com.cn/cn/hs300.html",
  "holiday_calendar_url": "https://www.gov.cn/gongbao/2025/issue_12406/material/gwygb202532.pdf",
  "evidence": {
    "provider": "Cffex",
    "source_at": null,
    "observed_at": "<same as response.observedAt>",
    "batch_id": "cffex-equity-index-planned-delivery-2026-v2:09"
  }
}
```

`rule_url` is the product-specific standing CFFEX contract table, never a
claimed monthly notice. The IF, IH, IC, IM rule URLs respectively are
`https://www.cffex.com.cn/cn/hs300.html`,
`https://www.cffex.com.cn/cn/sz50gzqh.html`,
`https://www.cffex.com.cn/cn/zz500.html`, and
`https://www.cffex.com.cn/zz1000/`. The holiday-calendar URL is the official
2026 State Council holiday notice. The year is a reviewed checked-in schedule:
third Friday, shifted to the next ordinary trading day for February and June
2026 holidays. It is not recalculated from an arbitrary client-supplied year.
Abnormal non-trading or subsequent CFFEX decisions can change a date. Clients
must treat this as a conditional plan and obtain a later exchange confirmation
before treating it as an actual settlement fact. There is no automatic runtime
override in this version; a changed date requires a reviewed source revision,
new tests and deployment.

This v2 response authorizes **planned-calendar display only**. It must not
authorize a same-day "official notice", "confirmed delivery", or settlement
fact push. `sourceAt` remains absent because the standing rule and holiday
calendar have different publications; inventing a unified monthly source time
would imply a notice that does not exist. A downstream confirmed-event product
must stay fail-closed until it obtains and validates a separate month-specific
exchange confirmation. The static service does not detect extraordinary market
closures; operators must review CFFEX changes and ship a new revision before
using an adjusted plan.

The [IM detailed rule](https://www.cffex.com.cn/cn/ssxz/20220718/43093.html)
includes the abnormal-closure condition; this public bundle
contains the minimal client contract, while the repository maintains the
full research note at `docs/integrations/research/cffex-futures-delivery-2026.md`.

Failure examples (gRPC status, not successful empty responses):

```text
schema_version=1: INVALID_ARGUMENT, FuturesDelivery requires schema magic.market.futures_delivery.request version 2
{"month":9}: INVALID_ARGUMENT, missing field `year`
{"year":2026,"month":13}: INVALID_ARGUMENT, futures delivery month must be in 1..=12
{"year":2027,"month":9}: UNIMPLEMENTED, formal CFFEX futures delivery is admitted only for 2026; requested 2027
```
