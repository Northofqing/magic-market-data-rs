# Magic TDX Board-Membership Provider Design

**Status:** Gate A approved

**Rule:** BR-026

**Scope:** existing Core `BoardMembershipProvider` implemented by production Magic TDX

## 1. Goal and non-goals

Expose exact industry, concept, and index membership from Magic TDX block files through
the existing provider-neutral `BoardMembershipProvider`. The implementation retains one
atomic three-file evidence batch and never infers a category from board title text.

This work does not add `BoardCatalog`, change Core records, join Eastmoney identifiers,
or modify the existing generic Router adapter. The Router already accepts any
`BoardMembershipProvider`; admission belongs in the TDX provider.

## 2. Existing contracts and source facts

- Core already defines `BoardMembershipProvider`, `BoardMembership`, `BoardCategory`,
  `SourceEvidence`, and `DataBatch`.
- `BlockService` already owns the blocking `TdxBlockClient`.
- TDX exposes `block_fg.dat`, `block_gn.dat`, and `block_zs.dat`.
- Each file supplies an exact block name, member stock code, file metadata size, and
  source hash. It supplies no board code and no publication timestamp.
- Core has no Index category. Under the existing Core rule, an unsupported source type
  remains `Unknown`; it is not relabeled Region by name.

The canonical board code is the lossless composite
`tdx:<source filename>:<exact source block name>`. It is stable, source-backed, and does
not pretend that TDX supplied an independent numeric board identifier. Batch identity
uses the exact source hashes of all three files. `source_at` remains absent.

## 3. Public interface

`BlockService` implements the existing trait:

```rust
impl BoardMembershipProvider for BlockService {
    type Error = TdxError;

    fn board_memberships(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<BoardMembership>, TdxError>;
}
```

No second provider or catalog interface is introduced. A private snapshot-source seam
allows deterministic fixture tests without changing production API.

## 4. Atomic data flow

```text
ordered requested instruments
  -> validate non-empty Shanghai/Shenzhen equities
  -> stable-collapse exact duplicates
  -> reject same code with conflicting identity
  -> BlockService/TdxBlockClient
       -> one bounded connection/handshake for stable block_fg.dat snapshot + source hash
       -> one bounded connection/handshake for stable block_gn.dat snapshot + source hash
       -> one bounded connection/handshake for stable block_zs.dat snapshot + source hash
  -> reject any missing/partial/empty/version-changing file
  -> validate every source record
  -> exact stock-code intersection with request
  -> normalize exact file/name/category evidence
  -> stable dedup and canonical order
  -> one strict DataBatch with shared provenance
```

A file snapshot reads metadata before and after its bytes. Size or hash changes reject
the file, preventing a batch from mixing versions. The three stable hashes form one
batch ID. All chunks for one file reuse the same bounded connection and handshake;
opening a fresh connection for every 30KB chunk is forbidden because per-request
timeouts would accumulate into an unbounded end-to-end delay. File reads remain
sequential and blocking inside the synchronous service; no Tokio runtime is created or
dropped.

## 5. Admission rules

### Request

- Empty request: invalid request.
- Only six-digit Shanghai/Shenzhen equities are supported.
- Beijing: explicit `Unsupported`; no Shanghai/Shenzhen remapping.
- Non-equity: explicit `Unsupported`.
- Exact duplicates: keep the first request position.
- Same code with different exchange or asset class: conflicting identity error before
  transport.

### Source

- All three files must be non-empty and stable across metadata checks.
- Filename is fixed by the provider; callers cannot supply it.
- Block name must be non-empty and control-free.
- Member code must be exactly six ASCII digits.
- Parsed block type must be the source-supported value `2`.
- A source file failure or invalid row rejects the whole batch.

### Normalization

- `block_fg.dat` -> `BoardCategory::Industry`.
- `block_gn.dat` -> `BoardCategory::Concept`.
- `block_zs.dat` -> `BoardCategory::Unknown`.
- Board code -> `tdx:<filename>:<exact blockname>`.
- Board name -> exact block name.
- Equivalent memberships collapse stably.
- Any conflicting normalized identity rejects the whole batch.
- Output order is request first-seen order, then Industry/Concept/Unknown, then board
  code/name.

If the complete three-file snapshot has no membership for the supported request, return
`DataBatch::strict(Vec::new(), provenance)`. This is complete empty evidence, not
provider unavailable.

## 6. Evidence

Batch provenance:

- source: `tdx-block-files`;
- source_at: absent because the protocol supplies no time;
- fetched_at: one local observation captured after the three snapshots;
- batch_id: domain-separated composite of `block_fg.dat`, `block_gn.dat`,
  `block_zs.dat` and their exact source hashes.

Every record uses `ProviderId::Tdx`, the same observed time and batch ID, and absent
record `source_at`. No field uses observed time as provider time.

## 7. Failure modes and rollback

Connection, timeout, parse, empty file, short/partial file, metadata version change,
invalid row, unsupported request, or identity conflict is an explicit `TdxError`.
There is no fallback, fuzzy matching, default board, or partial success.
The caller-supplied positive finite timeout bounds TCP connection establishment as well
as socket reads and writes; an unreachable TDX server cannot leave the synchronous
board-membership probe waiting for the operating-system default connect timeout.

Rollback reverts the scoped BlockService/provider, snapshot, tests, probe, BR, and docs
changes. It does not delete source files or alter any Core/Router contract.

## 8. Validation

Fixture tests cover request binding, exact categories, stable order/dedup, conflicts,
complete-empty evidence, source file atomicity, Beijing/non-equity rejection, and shared
evidence. A bounded production probe requests exactly `600396` (Shanghai) and `000001`
(Shenzhen), prints only typed membership/evidence fields, and fails unless both
instruments are represented or a complete source-backed empty result is returned.

Required gates:

```bash
cargo fmt --all -- --check
cargo test -p magic-tdx-rs --all-targets --locked --offline
cargo test -p magic-market-router --test tdx_board_memberships --locked --offline
cargo clippy -p magic-tdx-rs -p magic-market-router --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p magic-tdx-rs --no-deps --locked --offline
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
git diff --check
```

The bounded real probe requires network and is recorded separately from deterministic
offline tests.
