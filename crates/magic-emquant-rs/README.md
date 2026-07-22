# magic-emquant-rs

Read-only Eastmoney/Choice EMQuant adapter. It invokes the audited C++ snapshot
bridge as a subprocess so the Rust workspace remains free of `unsafe` code.

The bridge reads SDK paths and credentials from environment variables. This
crate never accepts, stores, or logs credentials. Provider capabilities must be
enabled only after the account's corresponding product permissions are verified.

Build the bridge, then run the live probe:

```bash
tools/emquant/build_snapshot_bridge.sh /path/to/EMQuantAPI_CPP_Mac
MAGIC_EMQUANT_BRIDGE=target/emquant/emquant-snapshot \
MAGIC_EMQUANT_LIB=/path/to/libEMQuantAPIx64.dylib \
MAGIC_EMQUANT_SERVER_LIST=/path/to/sdk/x64/bin \
cargo run -p magic-emquant-rs --example live_probe --release
```

`MAGIC_EMQUANT_CODES` optionally selects comma-separated `CODE.SH`/`CODE.SZ`
instruments. It defaults to `600519.SH,000001.SZ`. The probe prints every
normalized quote and all five bid/ask levels together with source and fetch
provenance. It also prints the latest five unadjusted daily OHLCV bars for the
first instrument. Authentication or entitlement failures are returned as
errors; the adapter never substitutes fixture data.

`HistoricalBars` currently uses the official `csd` API for day, week, month,
and year bars. Responses must be non-empty, strictly ascending, code-complete,
and OHLC-consistent. The adapter requests unadjusted prices explicitly and
returns at most the requested limit.

The bundled Mac SDK's declared `chmc` API supplies raw minute OHLCV records.
The adapter exposes 1/5/15/30/60-minute intervals and builds intervals above
one minute locally from consecutive raw records. It rejects reversed records
and gaps inside an aggregation bucket. `chmc` entitlement still needs a live
authorized account check because the current public online manual omits this
bundled API.

`MoneyFlows` queries the documented daily super-large/large/medium/small order
inflow and outflow fields through `css`, computes each net amount, and defines
main net flow as super-large net plus large net. Missing components remain
`Unavailable` and make the batch incomplete. This is a daily Choice indicator
contract, not a claim of five-second intraday money flow.

Opening-auction snapshots remain explicitly `Unsupported`: the verified
indicator set does not contain the matched price plus matched and unmatched
buy/sell quantities required by the core contract.

The bridge call times out after 30 seconds by default. Set
`MAGIC_EMQUANT_TIMEOUT_SECS` to a positive integer to override it.

EMQuant 2.0 and later authenticate with the official `userInfo` activation
token beside `ServerList.json.e`; no username/password variables are needed.
`MAGIC_EMQUANT_USERNAME` and `MAGIC_EMQUANT_PASSWORD` remain optional and must
be supplied together only for legacy SDK compatibility.
