# Official macro, SEC and news admission evidence — 2026-07-29

This artifact records categories and outcomes only. It intentionally excludes
API keys, SEC operator contact text, article descriptions and response bodies.

| Source | Deterministic command | Production-client result | Records/source time | Admission | Residual reason |
| --- | --- | --- | --- | --- | --- |
| NBS | `cargo test -p magic-nbs-rs --all-targets --offline` | Passed; bounded landing probe returned 140,978 bytes | Diagnostic landing only | false | No supported machine-readable national/regional series contract proved |
| PBC | `cargo test -p magic-pbc-rs --all-targets --offline` | Passed; two live and three-call serial load passed | Each live returned 12 requested M2 months; Jan–Oct present, Nov–Dec explicitly missing; no page release time | true for cataloged 2024 money supply | Social financing, regional series and uncataloged years unsupported |
| CFETS | `cargo test -p magic-cfets-rs --all-targets --offline` | Passed; two live plus independent three-call Shibor/LPR/FX loads passed | Requested Shibor ON/1W, LPR 1Y/5Y, USD/CNY and 100JPY/CNY source dates retained | true for Shibor/LPR/official FX | DR007 unsupported; no other rate substituted |
| FRED | `cargo test -p magic-fred-rs --all-targets --offline` | Passed; credentialed run not recorded | Synthetic observations/release evidence | false | `FRED_API_KEY` admission absent |
| IMF | `cargo test -p magic-imf-rs --all-targets --offline` | Passed; exact bounded probe received HTTP 403 | Timezone-less revision is not source time | false | Explicit transport failure; no live/load admission |
| World Bank | `cargo test -p magic-worldbank-rs --all-targets --offline` | Passed; real indicator metadata envelope parsed, then failed on empty structured unit | No observation is promoted without a source unit | false | Mandatory structured unit unavailable and never inferred |
| SEC EDGAR | `cargo test -p magic-sec-rs --all-targets --offline` | Passed; identified run not recorded | Synthetic filing/acceptance evidence | false | `SEC_USER_AGENT` live/load admission absent |
| Xinhua Finance | `cargo test -p magic-xinhua-rs --all-targets --offline` | Passed; two live and three-call serial load passed | 13 + 13 live rows; load 39 rows; oldest returned source time retained | true | First page metadata only; article bodies/history unsupported |
| Yicai | `cargo test -p magic-yicai-rs --all-targets --offline` | Passed; two live and three-call serial load passed | 50 + 50 live rows; load 150 rows; oldest returned source time retained | true | First page metadata only; notes/media/history unsupported |
| Securities Times | `cargo test -p magic-stcn-rs --all-targets --offline` | Passed; two live and three-call serial load passed | 30 + 30 live rows; load 90 rows; paired source timestamps retained | true | First page metadata only; content/share/history unsupported |

Every previously reported Critical/Important validation finding was repaired
without weakening the source contracts. Independent final reviews of
Core/Router, transport/China sources, global macro/SEC, and all three news
sources each reported zero remaining Critical/Important findings. Release
integration still requires the complete deterministic Gates A–D on the final
tree.
