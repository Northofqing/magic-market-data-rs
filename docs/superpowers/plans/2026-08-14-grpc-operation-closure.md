# gRPC Production Operation Closure Plan

**Design:** `docs/superpowers/specs/2026-08-14-grpc-operation-closure-design.md`
**Status:** completed on 2026-08-14; 46 handlers and 8 exact blockers verified

## Tasks

1. [x] Add a strict normalized TDX `SecurityProfiles` provider for bounded
   Shanghai/Shenzhen equities and deterministic fixtures.
2. [x] Add bounded live/load probes and record exact evidence.
3. [x] Register Tencent field-level `SecurityMetadata` and TDX
   `SecurityProfiles` handlers with versioned canonical schemas.
4. [x] Keep the other eight operations as exact fail-before-I/O blockers.
5. [x] Update capability counts, integration documentation and deployment status.
6. [x] Run Gate C verification and exercise both RPCs against the release server.

## Completion rule

An operation is removed from the blocked set only when its exact handler,
schema, failure categories and live evidence pass. A reachable diagnostic or a
narrower operation is not completion for a wider contract.
