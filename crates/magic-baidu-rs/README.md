# magic-baidu-rs

Read-only Baidu Stock Connect daily K-line adapter for
`finance.pae.baidu.com`.

- Maps source-supplied MA5/MA10/MA20 into `TechnicalBar`.
- Marks the verified history as `Adjustment::Unadjusted`: the request has no
  adjustment selector, and a captured ex-dividend gap remains in the response.
- Preserves source `--` moving averages as `None`.
- Uses one request, accepts at most 2,001 source rows, and caps the response at 8 MiB.
- Requires a successful JSON response from the verified official HTTPS host and
  follows zero redirects.
- Rejects unsupported intervals/date selectors and exchange/code-prefix
  mismatches (`6` Shanghai, `0`/`3` Shenzhen, `4`/`8`/`920` Beijing);
  other `9xxxxx` codes are rejected rather than guessed.
- Clones share a serial request gate; production request starts are at least one
  second apart and the gate is held through the complete response read.
- Normalizes source share volume to lots (`shares / 100`).

```bash
cargo run -p magic-baidu-rs --example live_probe --release
MAGIC_BAIDU_LOAD_REQUESTS=2 cargo run -p magic-baidu-rs --example load_probe --release
```

The bounded load probe reports successes, failures, records, RPS,
p50/p95/p99/max latency, and every request error. It exits non-zero if any
request fails.
