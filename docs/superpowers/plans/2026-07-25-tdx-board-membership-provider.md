# Magic TDX Board-Membership Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use TDD and execute this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the existing Core `BoardMembershipProvider` on production Magic TDX using one atomic, source-evidenced three-block-file batch.

**Architecture:** `BlockService` remains the only public provider surface. `TdxBlockClient` gains a stable source snapshot primitive that proves one file did not change during download; a private normalization seam validates and joins all three snapshots to request-bound Core memberships.

**Tech Stack:** Rust, Magic TDX blocking protocol client, `magic-market-core`, existing `magic-market-router`

---

### Task 1: Stable block-file snapshot

**Files:**
- Modify: `crates/magic-tdx-rs/src/block/client.rs`
- Modify: `crates/magic-tdx-rs/src/block/types.rs`
- Test: `crates/magic-tdx-rs/src/block/client.rs`

- [ ] Add a fixture-driven failing test that rejects metadata size/hash changes around a download.
- [ ] Add `BlockFileSnapshot { filename, hash, records }` and a private snapshot query seam.
- [ ] Read metadata before and after bytes, reject changes, reject empty records, then return exact hash and parsed rows.
- [ ] Run `cargo test -p magic-tdx-rs block_snapshot --locked --offline`.

### Task 2: Request-bound provider normalization

**Files:**
- Modify: `crates/magic-tdx-rs/src/service/blocks.rs`
- Test: `crates/magic-tdx-rs/tests/board_memberships.rs`

- [ ] Write one failing public-contract test for exact 600396/000001 membership mapping and shared evidence.
- [ ] Implement a private snapshot-source seam and `BoardMembershipProvider for BlockService`.
- [ ] Validate request identity before I/O; stable-collapse exact duplicates and reject conflicting identities.
- [ ] Validate three complete snapshots and normalize exact filename/name/category fields.
- [ ] Stable-deduplicate equivalent memberships, reject conflicts, and canonical-sort by request/category/code/name.
- [ ] Return strict complete-empty evidence for a complete source snapshot with no requested matches.
- [ ] Run `cargo test -p magic-tdx-rs --test board_memberships --locked --offline`.

### Task 3: Failure behavior

**Files:**
- Test: `crates/magic-tdx-rs/tests/board_memberships.rs`

- [ ] Add a failing test proving Beijing and non-equity requests fail before snapshot calls.
- [ ] Add a failing test for missing/empty/partial source family rejection.
- [ ] Add a failing test for equivalent duplicate collapse and conflicting request/source identity failure.
- [ ] Implement only the validation required by each failing test and rerun the focused target after each cycle.

### Task 4: Existing Router registration

**Files:**
- Create: `crates/magic-market-router/tests/tdx_board_memberships.rs`

- [ ] Add a compile-time production registration test using `board_membership_source`.
- [ ] Add a fixture result test proving Router preserves provider provenance and complete-empty batches.
- [ ] Do not modify Router production code unless the existing generic adapter cannot accept `BlockService`.
- [ ] Run `cargo test -p magic-market-router --test tdx_board_memberships --locked --offline`.

### Task 5: Bounded real probe and docs

**Files:**
- Create: `crates/magic-tdx-rs/examples/board_membership_probe.rs`
- Modify: `crates/magic-tdx-rs/README.md`
- Modify: `README.md`

- [ ] Implement a probe fixed to Shanghai 600396 and Shenzhen 000001 with bounded output.
- [ ] Print provider/source/source_at/observed_at/batch_id plus exact code/name/category only.
- [ ] Run the probe once against one configured primary server and record the outcome.
- [ ] Update capability docs only with the proven probe state; retain explicit gap text if the server is unavailable.

### Task 6: Gates

- [ ] Run focused tests and `cargo fmt --all -- --check`.
- [ ] Run strict Clippy for Magic TDX and Router.
- [ ] Run Rustdoc warnings-as-errors for Magic TDX.
- [ ] Run documentation links and compliance scripts.
- [ ] Run `git diff --check` and review only scoped diffs.
- [ ] Do not commit or push; report exact commands, probe evidence, and any blocked release gate.
