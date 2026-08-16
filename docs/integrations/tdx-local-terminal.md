# TDX local-terminal Rust polling admission

Status: official loopback transport selected; production data families remain
blocked pending bounded live and shadow evidence.

## Product boundary

The local-terminal monitor is an optional Windows leaf executable. Existing
libraries, Providers, Router order and default features do not start it. When
the operator starts the leaf executable it discovers the terminal
automatically; there is no TDX installation-path or endpoint configuration.
Exactly one `TdxW.exe` must be running in the current interactive Windows user
and session. If none is running, the service remains waiting and does not start
the poller; it may emit bounded waiting-status frames to stdout. The implemented
service has no inbound caller listener in any state. Ambiguous or unverified
processes fail closed.

The source is read-only market-data infrastructure. Account, cash, position,
order, cancel and execution methods are outside its source tree and BR-035.
Existing public TDX TCP Providers remain unchanged.

## Selected zero-Python data path

The vendor documents a TQ-Local HTTP calling convention:

- exact endpoint `POST http://127.0.0.1:17709/`;
- request envelope `{ "id": ..., "method": ..., "params": ... }`;
- `method` and `params` have the same data semantics as documented TdxQuant
  functions; and
- a TQ-capable TDX client must already be running.

The 2026-06-05 vendor release notes additionally describe direct TQ data calls
without Python code. This is the selected Rust path. Rust sends a small,
explicit read-only method enum through the fixed loopback origin. It never
imports Python, loads a vendor DLL, resolves a native export or guesses an
undocumented callback ABI.

The local HTTP client disables environment proxies and redirects, accepts no
alternate host, port or path, uses no credentials or cookies, and applies
injected positive connect/read/write timeouts and request/response byte limits.
One synchronous poll is in flight at a time. A timeout, non-JSON response,
non-matching response ID, RPC error, schema drift, invalid field/unit, terminal
exit or observation-sequence gap is explicit; none becomes an empty success.
An endpoint accepting a request proves transport presence only and does not
admit a data family.

TDX installation path, discovery-helper path and endpoint are zero-config: the
server resolves the helper beside its own executable and the HTTP origin is
compiled closed. This does not create monitor defaults. The equity watchlist,
poll/discovery timeouts, request/response bounds, every rule revision/window/
threshold/cooldown, snapshot cadence, restart budget, diagnostic cycle bound
and event-size bound are all required inputs. Identity-recheck cadence, stdout
queue capacity, output shutdown timeout and the closed `stop` slow-consumer
policy are required too; the current Config accepts 38 switch/value pairs and
has no implicit values. Watchlist identity is explicit as
`EQUITY:SH|SZ|BJ:dddddd`; no code-prefix asset or exchange guess is allowed.
For each new terminal generation, the server additionally issues one exact
read-only `get_stock_list(market="5", list_type=0)` request before polling.
It requires a bounded, non-empty, duplicate-free list of canonical A-share
identities and exact membership for every watchlist entry. A six-digit code is
therefore never sufficient proof by itself, and an absent identity starts no
market poller.

Official references:

- <https://help.tdx.com.cn/quant/docs/markdown/mindoc-1hdhbmi50d038.html>
- <https://help.tdx.com.cn/quant/docs/markdown/mindoc-1cfsjkbf8f3is/TdxQuantVersion.html>

## Current local evidence

Read-only discovery on 2026-08-13 found one current-user/current-session
`TdxW.exe` and recorded the following candidate identity. These facts select
version behavior but do not by themselves admit market data:

| Artifact | Observed identity |
|---|---|
| `TdxW.exe` | x86-64; file/product version `1,0,0,1`; SHA-256 `58bd2117ec86e8c063639f7adae4218011bb93998e3d93dcd286672d1978736b` |

The exact loopback endpoint accepted a malformed request and returned a typed
JSON parse error, proving that TQ-Local was listening. `get_match_stkinfo`
returned a valid typed response. Ten successive `get_pricevol` requests returned
HTTP 200 in approximately 19--51 ms; the Rust diagnostic client subsequently
observed approximately 12 ms and mapped only `Now` and `Volume`. TQ documentation
defines行情成交量 as lots (`手`), so the Rust contract retains that source unit
and performs no implicit share conversion.

The thirteenth bounded live request used the second allowlisted method,
`get_market_snapshot`, for exact instrument `600396.SH` and requested only
`Amount`, `Now`, `Volume`, and `LastClose`. On 2026-08-13 it returned HTTP 200
with `Amount="127354.65"`, `Now="17.62"`, `Volume="735536"`, and
`LastClose="17.18"`. The installed vendor sample at
`PYPlugins/user/tdxdata_test.py` states that total amount is expressed in
ten-thousand units while quantities without a special note retain their normal
unit. Rust therefore converts that amount with checked decimal arithmetic, not
floating point, to `1273546500` CNY and retains volume as `735536` lots. The
captured response is the parser fixture
`crates/magic-tdx-local-rs/tests/fixtures/tq_market_snapshot_success.json`;
extra vendor fields remain bounded transport evidence and are not normalized.
The response has no source timestamp or source record count, and `ItemNum` is
not relabelled as either fact.

On 2026-08-13 the third allowlisted read method, `get_stock_list`, returned
HTTP 200 in 264 ms with a 66,671-byte body containing 5,552 unique A-share
identities. It contained exact `600396.SH` and `000001.SZ`, excluded index
identity `000001.SH`, and excluded nonexistent `999999.SH`. A separate bounded
negative probe demonstrated why this gate is mandatory: asking `get_pricevol`
for `999999.SH` returned the Shanghai Composite values while labeling the map
with the requested key. Runtime therefore validates the watchlist against the
complete A-share universe before accepting any price/volume/snapshot response.

An earlier cold one-symbol snapshot completed in about 11 seconds, while the
captured bounded request above completed in under one second. This range is not
a production cadence, so amount remains an independently paced shadow family.
A separate one-symbol daily-bar request returned no bytes before a 10-second
diagnostic deadline; that historical-path result remains a timeout, not a
latency budget.

A later bounded monitor end-to-end run completed all 12 configured scheduler
cycles and exited zero. Discovery retained PID 31472, numeric version `1.0.0.1`
and the same executable SHA-256 above. The run observed price `17.18`
CNY/share, cumulative volume `1447695` lots and snapshot cumulative amount
`2520326100` CNY. Snapshot price/volume cross-checks against the latest fast
sample were both true; price, volume and amount monitors each reached
`warmed_up`. Every admission field remained false, and shutdown joined the
snapshot worker. The ignored local capture
`target/tdx-monitor-e2e.frames` used the documented four-byte big-endian frame
format. This proves one bounded diagnostic run, not restart/calendar/shadow
closure or production admission.

A 2026-08-13 afternoon-open diagnostic then completed all 6,000 configured
cycles and decoded 18,287 framed events without truncation. After a live source
increment, `600396.SH` volume triggered on a 11,617-lot window delta and amount
triggered on a 20,369,100-CNY delta; both entered cooling and rearmed at zero.
The second instrument did not inherit either transition. Two transient TQ
timeouts returned the service to Waiting; rediscovery, A-share-universe
validation and new-generation polling recovered automatically. These are
diagnostic lifecycle results and all admission markers remained false.

A later 3,000-cycle, 16-equity live diagnostic superseded the earlier
static-price captures. It ran for 1,389.5 seconds and exited zero on its own.
The 30,757,185-byte output decoded into 48,212 complete frames: 47,989 fast
observations, 29 snapshots and 174 analysis updates, followed by
`diagnostic_completed` and a joined snapshot worker. Two real loopback timeouts
caused explicit reset/waiting events; rediscovery and A-share-universe
revalidation recovered into three total generations.

Observed TQ batch refreshes completed 11 price and 15 cumulative-volume
`triggered -> entered_cooling_down -> rearmed` lifecycles. All 16 instruments
warmed independently in each generation, while transitions were limited to the
instruments whose exact windows met the injected rule. Example price changes
were `600050.SH` `4.33 -> 4.35`, `600396.SH` `17.51 -> 17.76`, and
`600519.SH` `1354.82 -> 1358.01`. The tiny diagnostic thresholds exist only to
exercise lifecycle transitions; they are not defaults or production advice.
Every admission marker remained false.

On 2026-08-15 the production admission decision was made for the three exact raw
observation families after the bounded shadow runs above. Three new serial
`get_pricevol` reads all returned `Now="16.99"` and `Volume="2835626"` with
approximately 19--30 ms call latency. Three serial `get_market_snapshot` reads
all returned the same price/volume plus cumulative amount `4962294700` CNY with
approximately 16--22 ms call latency. These are observation-time values; the
responses still contain no source timestamp or source record count. The admitted
scope is therefore current price in CNY/share, cumulative volume in lots and
cumulative amount in CNY only. Strict source freshness is not claimed.

The rebuilt Windows Server/Agent/monitor pair was then exercised through the
authenticated gRPC endpoint with `EQUITY:SH:600396` and `EQUITY:SZ:000001`.
Listener state was `agent_connected_production` and advertised exactly the three
families above. A bounded replay contained 846 admitted fast observations and 84
admitted snapshots in generation
`00003ebc-0000-4000-818c-f369f1e24f04:1`; the server-side tests separately reject
an admitted LocalAnalysis payload.

On 2026-08-16 the rebuilt Server/Agent/monitor chain verified the external
LocalAnalysis envelope contract against the running TDX terminal. A bounded
1,000-event replay contained 985 admitted raw observations/snapshots and 15
unadmitted analysis updates. Every analysis update passed the exact requested
instrument filter, carried a parseable monitor observation time, declared
`local_observation_time`, and repeated the same instrument inside its canonical
payload. The observed transitions were `warmed_up` and explicit resets during a
non-trading Sunday; no price/amount/volume trigger was fabricated from static
off-session values. This closes external message-time and identity plumbing,
not production threshold or trading-calendar admission.

The earlier unchanged samples and an ineffective immediate `refresh_cache`
call still establish that this is an interval batch source, not a
source-timestamped tick stream. The wrapper excludes `tick` in its market-data
period allowlist and its functional subscription path is the rejected
CPython/GIL callback. No tick-latency or strict source-freshness claim is made.
Continuous lunch polling regression-tests `Break -> Afternoon` as
`midday_break`, and every rule reset frame carries its exact reason (for example
`sampling_gap`). Post-open `get_market_data(period=1m,count=5)` diagnostics for
`600396.SH` and `300059.SZ` returned RPC success with empty values, so local
minute bars are not a separate fallback.

## Discovery unsafe boundary

All transport and protocol code is safe Rust. The bin-only
`magic-tdx-native-bridge` keeps one diagnostic Windows exception in
`discovery.rs` for read-only process/session/user and file-hash evidence. It
does not load a DLL or call a vendor native function. The root workspace and
all safe crates continue to forbid unsafe code.

`magic-tdx-native-bridge --discover` returns one bounded diagnostic JSON line.
The monitor server stdout contract is different: every `ServiceEvent` is UTF-8
JSON preceded by its four-byte big-endian `u32` payload length. It is binary
framing, not JSON Lines. The versioned frame codec in `magic-tdx-local-rs`
remains a protocol/test primitive and is not a claim that production market
data flows through the discovery helper.

## Monitor server and package boundary

`magic-market-monitor-server` serializes fast `get_pricevol` calls through the
main scheduler. The slower independently paced `get_market_snapshot` amount
request uses a capacity-one worker and an explicit cadence. Busy work is a typed
backpressure event; snapshot failure clears only the amount window and does not
advance or replay price/volume state. The captured amount is converted exactly
to CNY before analysis, while volume retains lot units.

Discovery evidence retains numeric file/product version and the version source
when Windows supplies them. A structured version-read failure remains evidence
instead of being replaced with a guessed display version. The service repeats
process identity discovery at an explicit cycle cadence; PID/session/creation
identity replacement resets affected windows and starts a new generation.

Stdout uses a bounded producer queue and a dedicated writer. The only accepted
slow-consumer policy is `stop`: a full queue, writer failure or explicit
shutdown timeout stops the service. Frames are not dropped and the polling
loop is not blocked to manufacture a delivery guarantee.

On a Windows host, `tools/release/package.sh` builds and installs
`magic-market-monitor-server.exe` and `magic-tdx-native-bridge.exe` into the same
`bin/` directory. Non-Windows hosts build neither. The recursive `bin/` hash
manifest covers both. Neither binary auto-starts or opens an inbound listener.
The price, cumulative-volume and cumulative-amount fields are production-admitted;
source-record count and every LocalAnalysis event remain unadmitted.

Each LocalAnalysis frame nevertheless carries the monitor-captured
`observed_at_utc` of the triggering local observation plus the explicit
`time_basis=local_observation_time`. The Agent copies this timestamp into the
external event envelope instead of substituting Agent receive or gRPC delivery
time. `source_at` remains absent because message time is not provider source
time. The frame repeats the exact canonical instrument at top level so external
instrument filters cannot collapse analysis events into a terminal-wide label.

## Blocked capabilities

LocalTerminal OHLC/previous-close and source-record-count capabilities remain
false. The source does not provide source timestamp or record-count semantics,
so strict source freshness remains unavailable. LocalAnalysis anomaly families
remain false until production thresholds and an authoritative trading-calendar
session policy are approved. Level-2/order/queue data,
full-market production monitoring, remote binding, raw retention, Webhook,
durable broker and production replay/restart defaults are also false or absent.

The compatibility matrix records executable provenance and observed TQ-Local
schema evidence; it does not independently promote a market-data family. An
unknown or updated executable hash is recorded and subjected to the same
fixed-origin bounded health/schema probe. Compatible responses may run without
user path/version configuration; schema or unit drift fails closed. Admission
remains limited to the three raw families recorded above.
