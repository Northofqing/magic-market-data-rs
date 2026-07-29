# Release-profile comparison evidence

## Identity and protocol

- Source revision: `8c8e9b5587ac48f4070e2524ea28fd4510836c77`
- Rust: `rustc 1.95.0 (59807616e 2026-04-14)`
- Cargo: `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`
- Platform: Darwin 25.5.0 x86_64
- Runner: one warm-up per profile; five alternating measured rounds
- Default: Cargo release defaults (`lto=false`, `codegen-units=16`)
- Candidate: `lto="thin"`, `codegen-units=1`
- Default binary: 663,992 bytes
- Candidate binary: 631,792 bytes

The runner required a clean worktree and index before building and verified
that the full revision, tracked worktree and index remained unchanged before
evidence collection and before exit. Every record included a finite positive
throughput derived from the fixed iteration count and elapsed nanoseconds.
Checksums were identical across all profiles and runs.

## Raw elapsed nanoseconds

| Profile | Workload | Iterations | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Checksum |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Default | `tdx_bar_parse` | 20,000 | 1,485,125,187 | 1,599,510,410 | 1,628,398,943 | 1,651,573,619 | 1,585,595,605 | 4,287,391,093,950,792,928 |
| Candidate | `tdx_bar_parse` | 20,000 | 1,591,715,421 | 1,531,126,558 | 1,479,790,026 | 1,567,802,842 | 1,647,245,191 | 4,287,391,093,950,792,928 |
| Default | `json_normalize` | 10,000 | 1,279,817,306 | 1,205,757,116 | 1,187,194,005 | 1,191,758,712 | 1,192,383,400 | 7,267,965,373,649,679,376 |
| Candidate | `json_normalize` | 10,000 | 1,183,764,686 | 1,108,944,121 | 1,162,488,584 | 1,067,826,539 | 1,216,816,030 | 7,267,965,373,649,679,376 |
| Default | `zlib_decompress` | 5,000 | 699,197,234 | 700,987,481 | 750,593,976 | 709,281,762 | 728,911,285 | 440,610,000 |
| Candidate | `zlib_decompress` | 5,000 | 750,008,534 | 730,768,802 | 723,514,357 | 681,723,583 | 737,210,544 | 440,610,000 |
| Default | `zlib_roundtrip` | 2,000 | 4,846,537,920 | 4,851,002,900 | 4,838,556,299 | 4,862,854,712 | 4,819,821,369 | 197,516,000 |
| Candidate | `zlib_roundtrip` | 2,000 | 4,671,619,130 | 4,534,909,658 | 4,730,543,092 | 4,748,347,997 | 4,674,214,327 | 197,516,000 |

## Qualification decision

| Metric | Observed | Required | Result |
| --- | ---: | ---: | --- |
| Combined median improvement | 1.285979% | at least 5% | fail |
| Largest workload regression | 3.029408% | at most 5% | pass |
| Binary growth | -4.849456% | at most 20% | pass |

The candidate is not qualified because the combined improvement is below the
fixed minimum. The repository therefore retains Cargo's default release
profile; no optimization claim is inferred from binary size alone.
