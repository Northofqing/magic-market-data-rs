# Concurrent Realtime Quote Routing Implementation Plan

**Goal:** Race admitted quote Providers, add HITHINK observation-time quotes,
and always retain the winning Provider and truthful time evidence.

**Seams:** `HithinkClient: RealtimeQuotes` is the Provider interface;
`OperationRegistry::execute` is the routing interface.

## Vertical slice 1: concurrent registry route

- Add a public-interface red test proving an unpinned quote request returns a
  fast second registration while the first remains blocked.
- Add bounded per-Provider race admission and deterministic all-failure details.
- Preserve the existing pinned-provider behavior and test it explicitly.

## Vertical slice 2: HITHINK adapter

- Add red fixture tests for exact URL construction, two-record normalization,
  optional batch timestamp, missing record source time and identity/cardinality
  rejection.
- Add only `/api/a-share/prices/snapshot` to the existing fixed endpoint policy.
- Implement the Core `RealtimeQuotes` interface and strict atomic normalization.

## Vertical slice 3: production composition

- Register `HithinkFinance / RealtimeQuotes` when the existing API key is loaded.
- Add registry and external serialization tests proving `selected_provider`,
  record Provider, `status`, `observed_at` and nullable `source_at`.
- Update the business rule, admission registry and integration/client docs.

## Vertical slice 4: live admission and release

- Run two bounded live probes and a three-call serial load probe without logging
  the credential or unrestricted payloads.
- Run formatting, focused/workspace tests, Clippy, compliance, documentation
  links and release preflight.
- Rebuild the runtime binary and client bundle, restart the service, then verify
  an unpinned live request selects the first data-bearing Provider.
