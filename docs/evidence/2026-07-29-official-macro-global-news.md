# Official macro, SEC and news admission evidence — 2026-07-29

This artifact records categories and outcomes only. It intentionally excludes
API keys, SEC operator contact text, article descriptions and response bodies.

| Source | Deterministic command | Production-client result | Records/source time | Admission | Residual reason |
| --- | --- | --- | --- | --- | --- |
| NBS | `cargo test -p magic-nbs-rs --all-targets --offline`; formal rerun 2026-08-13 | Passed; anonymous dynamic catalog/indicator/area/data path; two live and three-call load passed per exact scope | Each live returned July 2026 national headline CPI YoY `100.5%` and Beijing `100.2%`; no independent source timestamp | true only for exact national and Beijing CPI scopes | Every other geography/indicator/period fails before I/O |
| PBC | `cargo test -p magic-pbc-rs --all-targets --offline`; regional XLSX rerun 2026-08-13 | Passed; both exact scopes completed two live and three-call serial load | Money supply retains missing months; regional Q1 XLSX preserves 31 source-named regions, nine columns, 100-million-yuan unit and preliminary status | true for cataloged 2024 money supply and exact 2025Q1 regional AFRE flow | Other quarters, regional history and social-financing families unsupported |
| CFETS | `cargo test -p magic-cfets-rs --all-targets --offline` | Passed; two live plus independent three-call Shibor/LPR/FX loads passed | Requested Shibor ON/1W, LPR 1Y/5Y, USD/CNY and 100JPY/CNY source dates retained | true for Shibor/LPR/official FX | DR007 unsupported; no other rate substituted |
| FRED | `cargo test -p magic-fred-rs --all-targets --offline`; credentialed live/load rerun 2026-08-13 | Passed; two live runs returned four 2025 quarterly GDP rows each; three-call serial load passed | Official units/frequency/revision and observation evidence; key redacted | true | Exact v1 series contract only |
| IMF | `cargo test -p magic-imf-rs --all-targets --offline`; repeated 2026-08-13 | Passed; two live and three load attempts each received typed HTTP 403 | Timezone-less revision is not source time | false | Replacement SDMX Swagger requires beta portal login and a documented API contract |
| World Bank | `cargo test -p magic-worldbank-rs --all-targets --offline`; formal rerun 2026-08-13 | Passed; two live returned USA 2024 GDP `29298013000000`; three-call load passed | Official per-series metadata proves `current US$`, Annual and source revision | true only for source 2 / USA / `NY.GDP.MKTP.CD` / annual 2024 / max_rows=1 | Every other scope fails before I/O |
| SEC EDGAR | `cargo test -p magic-sec-rs --all-targets --offline`; identified live/load rerun 2026-08-13 | Passed; two live runs returned five Apple filings each; three-call serial load passed at one in-flight and >=500 ms spacing | Official filing/acceptance, subject CIK and submitting login-CIK evidence; contact redacted | true for metadata | Filing bodies, attachments and XBRL remain unsupported |
| Xinhua Finance | `cargo test -p magic-xinhua-rs --all-targets --offline` | Passed; two live and three-call serial load passed | 13 + 13 live rows; load 39 rows; oldest returned source time retained | true | First page metadata only; article bodies/history unsupported |
| Yicai | `cargo test -p magic-yicai-rs --all-targets --offline` | Passed; two live and three-call serial load passed | 50 + 50 live rows; load 150 rows; oldest returned source time retained | true | First page metadata only; notes/media/history unsupported |
| Securities Times | `cargo test -p magic-stcn-rs --all-targets --offline` | Passed; two live and three-call serial load passed | 30 + 30 live rows; load 90 rows; paired source timestamps retained | true | First page metadata only; content/share/history unsupported |

Every previously reported Critical/Important validation finding was repaired
without weakening the source contracts. Independent final reviews of
Core/Router, transport/China sources, global macro/SEC, and all three news
sources each reported zero remaining Critical/Important findings. Release
integration still requires the complete deterministic Gates A–D on the final
tree.

The final clean `workspace/all-features` production coverage report recorded
45,230 of 51,259 lines (88.24%) overall and 26,322 of 27,707 lines (95.00%) in
the configured critical data path. Both repository thresholds passed without
excluding production files or lowering a threshold. Stable-toolchain doctests
remain part of the ordinary release preflight; LLVM doctest persistence itself
is not used because that cargo-llvm-cov mode requires nightly Rust.
