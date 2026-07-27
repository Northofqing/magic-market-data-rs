# CFFEX delivery-calendar admission evidence — 2026-07-27

This artifact records bounded live admission attempts against the official
CFFEX notice directory. It is an exact failure record, not production
admission.

## Deterministic transport and parser contract

The explicit-backend transport, official URL allowlist, no-redirect rule,
response MIME/size bounds, timeout, shared pacing gate, typed TLS failure and
strict delivery parser passed:

```bash
cargo test -p magic-exchange-rs --locked --offline
```

Result:

```text
unit tests: 9 passed
cffex_transport: 6 passed (default) and 6 passed (`native-tls`)
capabilities: 1 passed
other exchange integration tests: passed (2 network-only tests ignored)
```

The formal `FuturesDeliveryCalendar` trait and diagnostic probe share the same
bounded internal fetch/parser operation. The formal trait does not execute that
operation while its capability is false.

## Rustls live attempt

Started at `2026-07-27T12:24:05+0800` (`Asia/Shanghai`):

```bash
MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery \
MAGIC_CFFEX_DELIVERY_YEAR=2026 \
MAGIC_CFFEX_DELIVERY_MONTH=7 \
MAGIC_CFFEX_TLS_BACKEND=rustls \
cargo run -p magic-exchange-rs --example live_probe --release --locked
```

The process exited `1` before receiving an HTTP response:

```text
provider=cffex-official
tls_backend=rustls
calendar_capabilities=CalendarCapabilities {
    economic_releases: false,
    futures_delivery: false,
}
Error: Tls { backend: Rustls, message: "https://www.cffex.com.cn/jystz/: Connection Failed: tls connection init failed: unexpected end of file" }
```

## Native TLS live attempt

Started at `2026-07-27T12:22:56+0800` (`Asia/Shanghai`):

```bash
MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery \
MAGIC_CFFEX_DELIVERY_YEAR=2026 \
MAGIC_CFFEX_DELIVERY_MONTH=7 \
MAGIC_CFFEX_TLS_BACKEND=native-tls \
cargo run -p magic-exchange-rs --example live_probe --release --locked \
  --features native-tls
```

The process exited `1` before receiving an HTTP response:

```text
provider=cffex-official
tls_backend=native-tls
calendar_capabilities=CalendarCapabilities {
    economic_releases: false,
    futures_delivery: false,
}
Error: Tls { backend: NativeTls, message: "https://www.cffex.com.cn/jystz/: Connection Failed: native_tls connect failed: connection closed via error" }
```

## Canonical path correction and repeat

The official HTTP directory proved that the old `/jystz/` entry now returns a
`301` to `/cn/jystz.html`. Its own paging JavaScript uses
`/cn/jystz_<N>.html`, and current detail links use
`/cn/jystz/<YYYYMMDD>/<ID>.html`. Because production disables redirects, the
allowlist and fixtures were corrected to those canonical `/cn` paths and now
explicitly reject the old paths.

The final repeated runs completed before
`2026-07-27T15:34:42+0800` against the corrected endpoint. The Rustls command
was:

```bash
MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery \
MAGIC_CFFEX_DELIVERY_YEAR=2026 \
MAGIC_CFFEX_DELIVERY_MONTH=7 \
MAGIC_CFFEX_TLS_BACKEND=rustls \
cargo run -p magic-exchange-rs --example live_probe --release --locked --offline
```

Its complete probe output after Cargo launched the binary was:

```text
provider=cffex-official
tls_backend=rustls
calendar_capabilities=CalendarCapabilities {
    economic_releases: false,
    futures_delivery: false,
}
Error: Tls { backend: Rustls, message: "https://www.cffex.com.cn/cn/jystz.html: Connection Failed: tls connection init failed: unexpected end of file" }
```

The Native TLS command was:

```bash
MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery \
MAGIC_CFFEX_DELIVERY_YEAR=2026 \
MAGIC_CFFEX_DELIVERY_MONTH=7 \
MAGIC_CFFEX_TLS_BACKEND=native-tls \
cargo run -p magic-exchange-rs --example live_probe --release --locked \
  --offline --features native-tls
```

Its complete probe output after Cargo launched the binary was:

```text
provider=cffex-official
tls_backend=native-tls
calendar_capabilities=CalendarCapabilities {
    economic_releases: false,
    futures_delivery: false,
}
Error: Tls { backend: NativeTls, message: "https://www.cffex.com.cn/cn/jystz.html: Connection Failed: native_tls connect failed: connection closed via error" }
```

Both still failed before HTTP. This rules out the stale path as the cause of
the remaining production failure while ensuring a recovered TLS route will use
the current official path.

## Admission decision

Neither backend reached the official notice HTML. Consequently, neither run
proved a July 2026 delivery date or the exact `IF2607`, `IH2607`, `IC2607` and
`IM2607` set. The result is:

```text
admission_state=failed_transport
calendar_capabilities.futures_delivery=false
formal_trait=Unsupported
```

## Plain-HTTP diagnostic (not admitted)

At `2026-07-27T13:11:40+0800`, a separate diagnostic showed that this host's
plain-HTTP route was reachable from the same machine even though every HTTPS
backend failed:

```bash
curl --http1.1 --connect-timeout 10 --max-time 20 \
  -A 'Mozilla/5.0' \
  http://www.cffex.com.cn/cn/jystz.html
```

The official directory returned `HTTP/1.1 200 OK` and linked
`/cn/jystz/20260717/48292.html`. The matching official detail page stated the
actual `2026-07-17` delivery and these settlement prices:

| Contract | Settlement price |
| --- | ---: |
| `IF2607` | `4555.58` |
| `IC2607` | `7593.39` |
| `IM2607` | `7265.90` |
| `IH2607` | `2841.64` |

This confirms the current official HTML schema and July values, but it does not
admit the Provider. Plain HTTP has no transport authenticity and is therefore
never used as an automatic fallback, never accepted by the production
allowlist, and never promoted to `source_at` evidence. Production remains
HTTPS-only and `Unsupported` until an HTTPS live run succeeds.

No third-Friday formula, cached calendar, browser result or third-party source
was substituted. Operators may explicitly choose either TLS backend for a
future diagnostic run; there is no silent backend fallback. Native TLS is an
optional crate feature, so the default Rustls build does not pull a system
OpenSSL dependency on Linux.
