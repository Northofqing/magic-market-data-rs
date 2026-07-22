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
provenance. Authentication or entitlement failures are returned as errors;
the adapter never substitutes fixture data.

The bridge call times out after 30 seconds by default. Set
`MAGIC_EMQUANT_TIMEOUT_SECS` to a positive integer to override it.

EMQuant 2.0 and later authenticate with the official `userInfo` activation
token beside `ServerList.json.e`; no username/password variables are needed.
`MAGIC_EMQUANT_USERNAME` and `MAGIC_EMQUANT_PASSWORD` remain optional and must
be supplied together only for legacy SDK compatibility.
