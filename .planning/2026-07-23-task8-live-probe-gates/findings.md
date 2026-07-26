# Findings

## Provider admission review

- Baidu currently advertises daily technical bars and its live probe prints
  `passed`, but the provider only validates request shape, non-empty ordered
  rows, OHLC/volume/amount and evidence. It does not prove the latest expected
  trading session, a trusted trading calendar, adjacent-close changes, or
  corporate-action continuity. Under the approved Gate A design it must remain
  `diagnostic_complete_unadmitted` until all four facts are source-proven.
- Baidu batch provenance uses the latest record date while each record carries
  its own date. The shared admission verifier deliberately requires batch and
  record source times to agree, so a multi-day Baidu batch cannot be admitted
  by the generic verifier without changing the evidence contract. This is
  another reason to keep the current live result diagnostic.
- CLS advertises only global news. A one-record live request can satisfy the
  shared verifier without conflating different publication times.
- CNInfo advertises announcements and investor questions. The current live
  probe prints debug batches but no stable status and needs one independently
  verified batch per advertised family.
- Existing load examples report configured client pacing as
  `min_interval_ms`; they do not measure actual high-level request-start gaps
  and therefore cannot truthfully claim a load admission result yet.

## Load-gate review

- The approved Gate A supplement is stricter than the original Slice 0 task:
  every advertised family must be exercised and the provider must report
  actual transport request starts plus observed concurrency. High-level call
  timestamps are insufficient, especially for CNInfo mapping/pagination and
  any Eastmoney family that may issue multiple requests.
- Existing Baidu, CLS and iWencai load examples print a configured
  `min_interval_ms` but never measure a start gap. THS only exercises
  popularity. CNInfo only exercises announcements. Eastmoney's default mixed
  mode covers four high-level operations, not every advertised family.
- A truthful load gate therefore needs shared, clone-visible instrumentation
  recorded inside each provider immediately before its production transport
  call, after internal pacing. The snapshot must expose actual start count,
  minimum start gap and maximum active transport calls without exposing
  secrets or test-only transport seams.
- The provider transports can share one Core tracker without changing the
  normalized data APIs. Eastmoney's injectable transport needs an optional
  telemetry method so a production load probe fails explicitly when a custom
  transport cannot supply evidence.
- High-level attempt count is not equivalent to transport request count.
  CNInfo and Eastmoney may perform mapping, pagination, or joins, so only the
  transport snapshot is authoritative for pacing and concurrency admission.
