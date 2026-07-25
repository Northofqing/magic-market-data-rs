# Coverage Gates

The release coverage check consumes the JSON emitted by `cargo llvm-cov` and
recomputes production line coverage from LLVM segments. It reads workspace
members from the root `Cargo.toml` and requires every
`<workspace-member>/src/**/*.rs` production source that LLVM can instrument.
Files outside the repository, missing files, unregistered source paths,
duplicate paths, omitted production sources, and omitted workspace targets are
rejected explicitly. Both Unix and Windows path separators are accepted.

Items directly attributed with `#[cfg(test)]` are removed from the production
line calculation, so inline unit tests cannot inflate the release percentage.
`#[cfg(not(test))]` remains production code. Other positive cfg expressions
containing `test` are rejected until their semantics are supported explicitly.
The small `NON_EXECUTABLE_SOURCE_PATHS` registry contains only current module,
constant, and re-export sources for which LLVM emits no executable segments;
it is intersected with the manifest-derived source set rather than acting as a
general path exclusion.

Two aggregate thresholds are enforced:

- all production files must have at least 80.00% line coverage;
- the combined critical set must have at least 95.00% line coverage.

The critical set contains files below a `codec`, `protocol`, or `adjustment`
directory, plus files named `service/common.rs` or `adapter.rs`. A configured
family contributes when matching source files exist in the repository. If such
a family exists but the report contains no measured matching file, the check
fails rather than silently reducing the critical set. Equality with either
threshold passes.

The report must contain exactly one export object, unique canonical file names,
valid integer line summaries, ordered six-field LLVM segments, and segment
locations within the current source length. Malformed reports and empty
production reports fail explicitly.

The final 2026-07-25 workspace report is:

```text
overall covered=22364 total=27931 percent=80.07 required=80.00
critical covered=1881 total=1960 percent=95.97 required=95.00
```

Run the same commands used by CI:

```bash
mkdir -p target/coverage
cargo llvm-cov --workspace --all-features --json \
  --output-path target/coverage/coverage.json -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

Run the checker contract tests with:

```bash
python3 -m unittest tools.coverage.test_check_thresholds
```
