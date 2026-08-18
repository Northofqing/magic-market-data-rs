# CFFEX plaintext notice diagnostic design

## Decision

Keep production `FuturesDelivery` disabled. Add one fixed-template diagnostic
that reads the official CFFEX notice directory over plaintext HTTP because a
real Microsoft Edge session on 2026-08-16 proved:

- `https://www.cffex.com.cn/cn/jystz.html` repeatedly fails before an HTTP
  response with `ERR_CONNECTION_CLOSED`;
- `http://www.cffex.com.cn/cn/jystz.html` returns the official server-rendered
  notice directory without login, cookies or XHR;
- `http://www.cffex.com.cn/cn/jystz/20260717/48292.html` returns the official
  July notice containing IF2607, IH2607, IC2607 and IM2607, their exact
  2026-07-17 delivery date and settlement prices.

This is a diagnostic availability path, not a transport-security exception for
production data.

## Fixed contract

The diagnostic has no endpoint input. It accepts only one validated year/month
request and scans at most 120 exact directory pages. The only permitted origin
is `http://www.cffex.com.cn`; paths are limited to:

```text
/cn/jystz.html
/cn/jystz_<2..=120>.html
/cn/jystz/<YYYYMMDD>/<numeric-id>.html
```

Requests are GET-only and reject credentials, ports other than 80, query,
fragment, redirect, cookies, authorization and bodies. Existing 1–60 second
timeout, 8 MiB body cap, UTF-8 HTML validation and minimum one-second shared
pacing remain in force. No retry changes schemes or origins.

## Evidence and output

The parser remains atomic and must prove the exact delivery-notice title,
publication date, requested month, all four equity-index futures contracts,
delivery date and settlement-price wording. The diagnostic batch identity is
domain-separated with `plaintext_http`. The returned canonical notice link is
the exact same official host/path normalized to HTTPS for reference, while the
capability blocker and integration evidence state that acquisition used
plaintext HTTP.

The formal Core trait remains `Unsupported`, repository admission remains
false, and gRPC returns `UNADMITTED`, `complete=false` and the plaintext blocker.
No browser profile, Cookie, TLS interception, local proxy or credential is used.

## Verification

- deterministic URL/mode tests reject cross-scheme, lookalike, credential,
  query, fragment, oversized and redirected responses;
- existing parser fixtures continue to prove four atomic records;
- one bounded live diagnostic proves the current public HTTP path;
- production trait and capability tests continue to prove fail-before-I/O;
- formatting, tests, Clippy, admission/HTTP compliance and documentation checks
  pass before release.

## 2026-08-17 result

The release provider probe returned exactly four records for `2026-07`:
`IF2607`, `IH2607`, `IC2607` and `IM2607`, all with delivery date and source
date `2026-07-17`. The deployed mTLS gRPC endpoint returned the same four
records with `ADMISSION_STATE_UNADMITTED`, `selectedProvider=Cffex` and a batch
ID containing `plaintext_http_diagnostic`. Repeating the same request with
`allow_unadmitted=false` returned `UNIMPLEMENTED` before provider I/O.
