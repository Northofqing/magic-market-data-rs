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
## BR-007 Security metadata evidence
Exchange is represented explicitly, including Beijing. Board derivation is
reported as derived evidence; listing dates and price-limit rule versions stay
unavailable unless the provider supplies them. Beijing is never mapped to an
unverified Shanghai/Shenzhen provider market number.
## BR-008 Financial archive integrity
Financial manifest sizes are bounded allocation hints. HTTP framing plus ZIP
entry bounds, uncompressed size, and CRC are mandatory before parsing or cache
admission.
## BR-009 Public-provider capability admission
An optional public-web capability is advertised only after deterministic
contract tests and a bounded live probe both prove the normalized records,
source identity, source time when supplied, observation time and batch
identity. Authentication-gated or unverified families remain false and return
a typed `Authentication`, `Unsupported` or protocol error.

## BR-010 Public-provider request bounds and pacing
Every public-web request enforces its verified positive row bound before I/O.
Clones of one Provider client share the same request limiter. Where the source
contract requires pacing, request starts are serialized at no less than the
documented interval; HTTP 429 and limiter failure are explicit errors and do
not trigger an unpaced retry.

## BR-011 Public-provider duplicate identity
Within one atomic Provider batch, duplicate business identities are rejected
as protocol failures. The only admitted exception is semantic-search output:
rows with the same normalized security identity are collapsed to the
source-supplied highest score, with deterministic first-seen tie breaking.
No downstream consumer may deduplicate by display name.
