# Unpinned Rust Toolchain Design

**Status:** Gate A approved for implementation  
**Date:** 2026-07-23  
**Scope:** workspace build, CI, release preflight and release packaging only

## Intent

The workspace must not select or declare one exact Rust release. Developers use
their current default toolchain, CI installs the current stable toolchain, and
release evidence records the compiler and Cargo versions that actually produced
an artifact. `Cargo.lock`, `--locked` and offline deterministic gates remain the
dependency-reproducibility boundary.

This change does not weaken market-data validation, source provenance,
freshness, explicit failure handling, provider capability admission or request
governance.

## Current problem

The repository currently selects Rust 1.83.0 in `rust-toolchain.toml`, declares
`rust-version = "1.83"` in the workspace, inherits that declaration in every
crate, invokes release gates through `RUSTUP_TOOLCHAIN=1.83.0`, and packages the
selection file. CI also carries a fixed-version MSRV job. These independent
constraints make a routine toolchain update a repository-wide maintenance task
and can disagree with the developer's or release runner's active toolchain.

## Design

### Toolchain selection

- Delete `rust-toolchain.toml`.
- Delete the workspace `rust-version` declaration and all
  `rust-version.workspace` inheritance.
- Keep Rust edition 2021. Edition is source-language semantics, not a toolchain
  selector.
- Do not introduce an MSRV claim elsewhere. A future MSRV may be added only as
  a separately designed compatibility policy with an actual lower-bound test.

### CI and release gates

- Every GitHub Actions Rust job installs `stable`; the fixed MSRV job is
  removed rather than renamed into a duplicate stable job.
- `tools/release/preflight.sh` invokes the runner's default `cargo` and `rustc`.
  It retains formatting, locked/offline all-feature checks and tests, strict
  Clippy, rustdoc, doctests, documentation links and compliance.
- `tools/release/package.sh` no longer requires, copies or hashes a toolchain
  selection file. It continues to write `RUSTC_VERSION` from `rustc -vV` and
  `CARGO_VERSION` from `cargo -V`, so every artifact remains traceable to its
  actual build toolchain.

### Compliance boundary

The compliance script rejects:

1. a repository-root `rust-toolchain` or `rust-toolchain.toml`;
2. `rust-version` in the root or crate Cargo manifests;
3. an exact numeric `dtolnay/rust-toolchain@...` selector in active workflows;
4. `RUSTUP_TOOLCHAIN=<numeric version>` in active build/release tooling.

The scan is deliberately limited to active manifests, workflows and tools.
Historical specifications, plans, changelog entries and performance evidence
may retain the exact compiler version under which that historical work ran.

## Failure modes

- **A dependency needs a newer compiler:** locked/offline workspace check fails
  explicitly. The lockfile must be reviewed; the gate must not silently select
  another dependency set.
- **The runner lacks Rust, rustfmt or Clippy:** the relevant command fails
  explicitly. Release tooling does not install components or mutate the host.
- **Stable introduces a lint or compiler regression:** CI/preflight fails and
  blocks release. Fix the code or, after a new Gate A decision, introduce a
  temporary controlled toolchain policy.
- **Artifact provenance is questioned:** inspect `RUSTC_VERSION`,
  `CARGO_VERSION`, `RELEASE_REVISION`, `TARGET_TRIPLE` and `SHA256SUMS` in the
  package.

## Validation

Run without installing or selecting another toolchain:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features --all-targets --locked --offline
cargo test --workspace --all-features --all-targets --locked --offline -- --test-threads=1
cargo clippy --workspace --all-features --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked --offline
cargo test --workspace --all-features --doc --locked --offline -- --test-threads=1
bash tools/compliance/check.sh
bash tools/release/preflight.sh
```

## Rollback

The design and implementation are separate commits. Revert only the
implementation commit to restore the previous selection mechanism; do not
rewrite package history or alter market-data code.
