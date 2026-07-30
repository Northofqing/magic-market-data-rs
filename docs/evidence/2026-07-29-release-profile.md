# Release-profile comparison evidence

## Session A identity and protocol

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

## Session A raw elapsed nanoseconds

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

## Session A qualification decision

| Metric | Observed | Required | Result |
| --- | ---: | ---: | --- |
| Combined median improvement | 1.285979% | at least 5% | fail |
| Largest workload regression | 3.029408% | at most 5% | pass |
| Binary growth | -4.849456% | at most 20% | pass |

The candidate is not qualified because the combined improvement is below the
fixed minimum. No optimization claim is inferred from binary size alone.

## Session B identity and protocol

- Source revision: `d9555c6b06bcb27360a98a13765b8d0051ff575a`
- Rust: `rustc 1.95.0 (59807616e 2026-04-14)`
- Cargo: `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`
- Platform: Darwin 25.5.0 x86_64
- Runner: one warm-up per profile; five alternating measured rounds
- Default: Cargo release defaults (`lto=false`, `codegen-units=16`)
- Candidate: `lto="thin"`, `codegen-units=1`
- Default binary: 664,120 bytes
- Candidate binary: 631,792 bytes

This session used the runner present at `d9555c6`. It required a clean full Git
porcelain state before the build and at every verification point, rejected
inherited environment variables matching `CARGO_*`, `RUST*`, and `SCCACHE_*`,
required the exact four-workload schema and tool-version formats, and
restricted in-repository artifacts to the ignored `target/` tree. It did not
isolate `$HOME/.cargo/config{,.toml}` or Cargo configuration in mutable
ancestor directories, and it did not build from the current read-only,
digest-checked snapshot. Checksums were identical across all profiles and
runs, but the session does not meet the current provenance standard.

## Session B raw elapsed nanoseconds

| Profile | Workload | Iterations | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 | Checksum |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Default | `tdx_bar_parse` | 20,000 | 1,701,724,876 | 1,649,739,624 | 1,606,342,731 | 1,645,003,943 | 1,589,817,428 | 4,287,391,093,950,792,928 |
| Candidate | `tdx_bar_parse` | 20,000 | 1,527,242,302 | 1,629,553,436 | 1,526,180,512 | 1,550,589,876 | 1,620,571,583 | 4,287,391,093,950,792,928 |
| Default | `json_normalize` | 10,000 | 1,326,694,303 | 1,258,431,039 | 1,231,540,054 | 1,207,483,807 | 1,305,240,786 | 7,267,965,373,649,679,376 |
| Candidate | `json_normalize` | 10,000 | 1,251,922,198 | 1,191,662,446 | 1,157,620,254 | 1,075,437,255 | 1,127,930,460 | 7,267,965,373,649,679,376 |
| Default | `zlib_decompress` | 5,000 | 906,011,166 | 861,454,796 | 866,127,773 | 788,377,283 | 774,919,572 | 440,610,000 |
| Candidate | `zlib_decompress` | 5,000 | 780,660,603 | 747,521,880 | 772,319,352 | 756,792,140 | 799,879,096 | 440,610,000 |
| Default | `zlib_roundtrip` | 2,000 | 4,934,009,963 | 4,982,991,366 | 4,969,610,920 | 4,890,685,738 | 5,038,138,221 | 197,516,000 |
| Candidate | `zlib_roundtrip` | 2,000 | 4,766,588,283 | 4,731,810,894 | 4,704,187,059 | 4,637,549,267 | 4,840,916,252 | 197,516,000 |

## Session B qualification decision

| Metric | Observed | Required | Result |
| --- | ---: | ---: | --- |
| Combined median improvement | 7.245705% | at least 5% | pass |
| Largest workload regression | -4.785083% | at most 5% | pass |
| Binary growth | -4.867795% | at most 20% | pass |

Session B passes the numerical comparison policy used by that historical
runner. It does not qualify under the current provenance standard.

## Cross-session repository decision

Session A and Session B were clean, offline measurements on the same machine
and toolchain, and their combined improvements were respectively 1.29% and
7.25%. They are not a repeatability experiment: Session B used a different
revision that changed the TDX parser hot path and hardened the runner, and the
default binary sizes differ. The two results cannot be compared to qualify a
workspace-wide release-profile claim. The evidence is insufficient, so the
repository retains Cargo's default release profile until independent sessions
at one exact revision consistently satisfy the policy.
