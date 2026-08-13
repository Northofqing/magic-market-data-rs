# Task Plan: P0 Architecture Hardening

## Goal
Deliver the approved first hardening slice: restore real synchronous TDX pool concurrency, document blocking-runtime boundaries, and prevent growth of unregistered provider-local HTTP stacks.

## Current Phase
Phase 5

## Phases

### Phase 1: Requirements & Discovery
- [x] Understand user intent from the architecture audit and approval
- [x] Decompose the broad audit into an independently releasable P0 slice
- [x] Compare three scope approaches and select the bounded hardening slice
- **Status:** complete

### Phase 2: Gate A Design
- [x] Write the P0 hardening design specification
- [x] Self-review the specification for placeholders, contradictions, and scope
- [x] Obtain user review of the written specification
- **Status:** complete

### Phase 3: Implementation Planning
- [x] Use writing-plans to produce the implementation plan
- [x] Review the plan against the approved specification
- **Status:** complete

### Phase 4: Implementation
- [x] Execute the approved plan with failing tests first
- [x] Run focused and workspace verification
- **Status:** complete

### Phase 5: Delivery
- [x] Run formatting, tests, Clippy, compliance, and documentation checks
- [x] Request code review and address findings
- [x] Restore the previously active planning session
- [x] Deliver to user
- **Status:** complete

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Implement only the P0 slice in this specification | Full transport migration, Router racing, and breaking API changes are independent failure domains |
| Keep ordered Router and all four TDX strategies unchanged | Both are intentional public contracts under existing Gate A decisions |
| Add a transport-boundary registry/check rather than mass migration | It stops debt growth while enabling evidence-backed provider-by-provider migration |

## Errors Encountered
| Error | Resolution |
|-------|------------|
| First combined patch used a wrapped design-doc context that did not match exactly | Re-read the exact lines and applied a targeted patch |
