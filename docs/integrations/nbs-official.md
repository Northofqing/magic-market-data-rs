# National Bureau of Statistics integration

## Capability state

National and regional economic-series capabilities are false. The current
adapter is a diagnostic parser/probe only; the formal Provider fails
`Unsupported` before I/O.

## Official host and paths

Only HTTPS resources on `www.stats.gov.cn` are permitted. The bounded landing
probe uses `/`; no unofficial mirror is a substitute.

## Request and response ceilings

Responses are capped at 4 MiB, diagnostic nodes are bounded, and request
starts are paced by one second.

## Identity, unit, missing, and source-time semantics

Provider-qualified series, regions, periods, units and source metadata remain
source facts. A numeric zero is present data; absent `datanode` coverage is a
protocol failure, never an implicit missing or zero.

## Authentication or usage-rights boundary

No login, Cookie, CAPTCHA bypass, browser-session extraction or protected
endpoint is used.

## Deterministic tests

Synthetic fixtures cover full-node validation, duplicate identities, zero,
missing coverage, evidence and I/O-free unsupported behavior.

## Live and load admission evidence

An earlier minimal-client audit returned HTTP 403. On 2026-07-29 the bounded
Rust landing-page diagnostic succeeded with 140,978 bytes, but no supported
machine-readable national or regional series contract was proved. Production
capabilities therefore remain false.

## Explicit unsupported operations

Production national/regional series, browser emulation, unbounded queries and
unverified datasets are unsupported.
