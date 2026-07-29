# Findings & Decisions

## Requirements
- Fix all confirmed issues from the audit, not the disproved claims.
- Preserve explicit failures, source evidence, Gates A-D, and one-way
  dependencies.
- Do not rewrite the checked `f64` value layer merely for exact equality.
- Do not add release optimizations without measured evidence.

## Research Findings
- TDX `read_u16`/`read_u32` and `get_price` silently return zero on
  truncation. Multiple production parsers accept partial declared batches.
- TDX tests currently accept a truncated security list as an empty success;
  the fuzz smoke test checks only absence of panics.
- The low-level TDX helper modules are public, so changing signatures is an
  explicit pre-1.0 API hardening that must be documented.
- Core value constructors and custom deserializers already reject NaN and
  infinity.
- Manual epoch-plus-eight-hour formatting is independent of host timezone,
  but the calendar conversion is duplicated across several Providers.
- Core post-close validation already parses and bounds the full clock, so the
  reported BR-019 bypass is not reachable through normalized records.
- The root manifest has no release profile; the claimed 10-30% benefit has no
  repository benchmark evidence.
- Exchange's local gate intentionally holds one mutex across pacing and full
  I/O. Its TLS backend choice and injected transport seam must survive any
  migration.
- Exchange is not the only Provider outside `magic-market-transport`; only the
  newer official/global Provider group consistently uses it.
- Current admitted PBC, CFETS, Xinhua, Yicai, and STCN families have documented
  live/load evidence. NBS, FRED, IMF, World Bank, and SEC remain explicitly
  unadmitted.
- Router production source is 3,074 lines, with 8,932 lines of tests. Its
  ordered first-acceptable-source behavior is deliberate and auditable.
- `magic-market-transport` already has cross-crate HTTP policy and gate tests.
- Existing numeric tolerances encode distinct source/business units and must
  not be replaced with one global epsilon.

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| Introduce one checked packet cursor for TDX | Central bounds/offset handling prevents each parser from reimplementing partial checks. |
| Require declared-count agreement | A complete empty response is distinct from a truncated nonempty response. |
| Reuse shared request-start pacing for Exchange | The shared gate releases reservation locks before waiting and I/O. |
| Keep an Exchange TLS adapter where required | Shared Rustls-only transport cannot silently replace the admitted native-tls diagnostic path. |
| Add a declarative admission registry checked by compliance tooling | Avoid provider dependencies in the Router while detecting flag/document drift. |
| Add parameterized checked tolerance primitives plus named call-site policies | Reuse mechanics without erasing units or source precision. |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| Some audit claims combined real code smells with false consequences | Separate factual presence, production reachability, and remediation priority. |

## Resources
- `docs/business_rules.md`
- `docs/ENGINEERING_RULES.md`
- `docs/PERFORMANCE_RESULTS.md`
- `tools/compliance/check.sh`
- Provider integration documents under `docs/integrations/`
# Implementation Findings

## TDX packet boundary

- Both security and index bar fixtures demonstrate a protocol-authorized
  optional four-byte tail. The strict parser therefore accepts exactly zero or
  four tail bytes and rejects every other tail length.
- A single checked cursor now distinguishes valid encoded zero from truncated
  or unterminated data. Public fixed-width readers and `get_price` return
  `Result`, so workspace callers can no longer silently manufacture zero.
- Security lists, declared bar batches, transaction batches, and realtime quote
  batches reject a missing later record atomically. Historical minute data has
  no count field, so its six-byte header is required and every started
  price/auxiliary/volume tuple must finish.
