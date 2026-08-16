# gRPC Provider Parity Implementation Plan

1. Add the append-only `InstrumentNews` operation to service, Protobuf,
   generated mapping, compatibility fixtures, and external documentation.
2. Add composition dependencies for already-implemented Providers; do not add
   HTTP/TLS dependencies. Record the one exact Sina `quotes.sina.cn` host
   allowlist correction in `http-transports.tsv` after the live parity probe.
3. Register admitted news and official economic Providers using existing
   request/record schemas and `provider_query_result` provenance handling.
4. Register TDX public, Sina, SZSE, and SSE alternatives for the exact families
   they already advertise.
5. Register EMQuant only as a bounded diagnostic when the managed bridge is
   discoverable; otherwise expose a typed unavailable capability.
6. Add deterministic registry tests for exact Provider/operation pairs,
   append-only enum mapping, fail-before-I/O behavior, and admission state.
7. Update README and the external gRPC integration guide.
8. Run format, targeted and workspace tests, Clippy, compliance, docs, release
   build, service restart, capability discovery, and representative live calls.
9. Add append-only operations 56 through 60 for `IndexQuotes`,
   `IntradayShape`, `T0Evidence`, `OutcomeDailyBars`, and
   `UpperLimitPoolReview`; publish their exact v1 schemas. Index, shape,
   exact-through outcome bars, and four-pool review are now admitted after
   deterministic plus 2 live/3 serial evidence. T0 returns its available
   four-family bundle only as an explicit incomplete diagnostic until public
   TDX Quote/book source time is proved.
