# magic-emquant-rs

Read-only Eastmoney/Choice EMQuant adapter. It invokes the audited C++ snapshot
bridge as a subprocess so the Rust workspace remains free of `unsafe` code.

The bridge reads SDK paths and credentials from environment variables. This
crate never accepts, stores, or logs credentials. Provider capabilities must be
enabled only after the account's corresponding product permissions are verified.

Build the bridge for the installed official SDK, then run the live probe.

macOS:

```bash
tools/emquant/build_snapshot_bridge.sh /path/to/EMQuantAPI_CPP_Mac
cargo run -p magic-emquant-rs --example live_probe --release
```

Windows x64:

```powershell
tools\emquant\build_snapshot_bridge_windows.cmd C:\path\to\EMQuantAPI_CPP
cargo run -p magic-emquant-rs --example live_probe --release
cargo run -p magic-emquant-rs --example daily_bars_probe --release --locked --offline
```

The builder always resolves paths relative to this repository and writes the
executable to `target/emquant/emquant-snapshot[.exe]` by default. It also installs the
encrypted server list, activator image assets, a project-local SDK library, and
a protected project-local copy of the activation file when present under ignored
`target/emquant/runtime`. On Windows, the builder locates the installed Visual
Studio x64 C++ toolchain and copies only the official x64 DLL, server list and
activator layout. The DLL is loaded from its absolute path with dependency
search restricted to its own directory and System32. On macOS,
the builder clears quarantine metadata and ad-hoc signs only the local library
copy; the vendor download remains unchanged. The Rust adapter and bridge discover
those project-local paths automatically. `MAGIC_EMQUANT_BRIDGE`,
`MAGIC_EMQUANT_LIB`, and `MAGIC_EMQUANT_SERVER_LIST` are optional deployment
overrides.

If the probe reports `10001014 (EQERR_NEED_ACTIVATE)`, run the prepared
`target/emquant/runtime/LoginActivator.exe` on Windows or
`target/emquant/runtime/loginactivator_mac` on macOS beside `ServerList.json.e` and
complete the official API activation flow. It writes `userInfo` into that
ignored runtime directory. A desktop Eastmoney login is a separate session and
does not activate EMQuant API access. The macOS activator requires GTK 3 and
uses the API account's bound mobile number plus a verification code; it is not
a username/password form.

`MAGIC_EMQUANT_CODES` optionally selects comma-separated `CODE.SH`/`CODE.SZ`
instrument. It defaults to `600396.SH,000001.SZ` (华电辽能、平安银行). The probe prints every
normalized quote and all five bid/ask levels together with source and fetch
provenance. It also prints the latest five unadjusted daily OHLCV bars for the
first instrument. Authentication or entitlement failures are returned as
errors; the adapter never substitutes fixture data.

`HistoricalBars` currently uses the official `csd` API for day, week, month,
and year bars. Responses must be non-empty, strictly ascending, code-complete,
and OHLC-consistent. The adapter requests unadjusted prices explicitly and
returns at most the requested limit.

The bundled SDK's declared `chmc` API supplies raw minute OHLCV records.
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

On 2026-08-21, a restored 15-day API entitlement completed two focused probes
and four serial SDK requests for `600396.SH` and `000001.SZ`. The admitted
production scope is deliberately narrow: Shanghai/Shenzhen equities,
`interval=Day`, explicit inclusive start/end dates, unadjusted completed `csd`
rows and at most 800 returned rows. Every returned row must contain valid OHLC,
volume, amount, date, provider, observation time and batch evidence.

Quote, five-level book and minute history still fail with `10001012`; the
current money-flow response contains instrument rows but no admitted numeric
fields. Those families remain diagnostic only. Repository admission does not
extend the temporary account entitlement: after it expires, the SDK error is
returned as a typed unavailable failure with no records. The adapter never
turns permission expiry into a successful empty market batch and never fills
zero, cached data or another provider.

The focused admission probe accepts optional
`MAGIC_EMQUANT_DAILY_CODES`, `MAGIC_EMQUANT_DAILY_START`,
`MAGIC_EMQUANT_DAILY_END` and `MAGIC_EMQUANT_DAILY_LIMIT` overrides.
