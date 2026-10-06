# Critical coverage test extraction

Date: 2026-10-06
Gate A scope: mechanical release-gate repair under the standing development,
remote-submission and gRPC coordination authorization. The existing coverage
contract in tools/coverage/README.md already requires path-based external tests.

## Frozen baseline and cause

The baseline is eea9cc6eea57725da1bc602dd66de68448c8d8ab. All previously sealed
eea artifacts and the running 4e service remain unchanged. The actual coverage
checker rejects inline critical test bodies before inspecting a report.

The four affected source modules are:

- magic-eastmoney-rs/src/mx.rs;
- magic-market-composition/src/derived_products.rs;
- magic-market-composition/src/grpc_production.rs;
- magic-nbs-rs/src/api.rs.

## Mechanical change

Move each existing tests module body to its crate's tests/internal directory,
and retain the original module identity and private access with
`#[cfg(test)] #[path = "../tests/internal/<name>_tests.rs"] mod tests;`.

Preserve every test, helper, fixture, assertion and test attribute. Remove
the outer module wrapper and use the repository formatter on its body. No new integration-test
crate is introduced. Existing super/crate references retain their context;
there are no location-dependent include/file/line macros in the moved bodies.

Production prefixes, public contracts, Provider admission, dependencies,
transport policy, runtime configuration and coverage policy are unchanged.
Do not alter the 80/95 thresholds, critical globs, checker, ignores or assertions.

## Verification

- Compare the extracted bodies byte-for-byte with the original inner bodies
  independently formatted as external files; verify LF-normalized production
  prefixes against the canonical eea Git blobs.
- Re-run the original critical-source structure check and record RED and GREEN.
- Run all three affected crates' library tests in a fresh independent target;
  preserve names and counts, formatting, Clippy, docs and compliance.
- Commit the exact repair candidate, run independent Standards and Spec review
  against eea, and publish only the feature branch without rewriting old refs.
- Trigger the existing security/release coverage workflow at the exact new
  commit. Retain the measured JSON, full threshold log, job identity and result.
  Coverage failure stays a release blocker; checker controls are not measurement.
- Create a new runtime candidate only after actual release gates are qualified.
  Prior eea Rust checks do not count as new-source verification.

## Runtime and rollback

No listener, real Provider/RPC, service stop/start, authentication, data owner,
keepalive, force-stop or production switch is part of this source slice.
Rollback is a new revert or candidate abandonment, never deletion or alteration
of the old source commits, sealed artifacts or existing user reports.
