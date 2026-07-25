# Coverage Gates

The release coverage check consumes the JSON emitted by `cargo llvm-cov` and
counts production line summaries only. A production file must be below
`crates/*/src`; paths containing `tests`, `examples`, `benches`, `fuzz`, or
generated `target` directories are excluded. Both Unix and Windows path
separators are accepted.

Two aggregate thresholds are enforced:

- all production files must have at least 80.00% line coverage;
- the combined critical set must have at least 95.00% line coverage.

The critical set contains files below a `codec`, `protocol`, or `adjustment`
directory, plus files named `service/common.rs` or `adapter.rs`. A configured
family contributes when matching source files exist in the repository. If such
a family exists but the report contains no measured matching file, the check
fails rather than silently reducing the critical set. Equality with either
threshold passes.

The report must contain exactly one export object, unique file names, and
integer line totals satisfying `0 <= covered <= count`. Malformed reports and
empty production reports fail explicitly.

Run the same commands used by CI:

```bash
cargo llvm-cov --workspace --all-features --json \
  --output-path target/coverage/coverage.json -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

Run the checker contract tests with:

```bash
python3 -m unittest tools.coverage.test_check_thresholds
```
