# Business rules
## BR-001 Quote request cardinality
Strict quote requests accept 1 through 60 instruments. `quotes_chunked` is the only API that may split a larger request.
## BR-002 Strict pagination
Strict paginated operations are atomic.
## BR-003 Pool and queue policy
Blocking defaults to five connections; async defaults to four.
## BR-004 Smart server policy
Smart selection orders eligible servers by observed health and latency.
## BR-005 Adaptive rate limits
The compatible schedule is 15/30/60 requests per second with a 200 request-per-second ceiling.
## BR-006 Cache policy
Caching is disabled by default and never relabels stale values as fresh.
