# Tonghuashun exact-date limit-pool completeness plan

1. Register BR-032 and document exact-date/whole-batch admission.
2. Add failing tests for response date and page-total contradictions.
3. Validate `data.date`, `data.page.{page,limit,total}` before mapping rows.
4. Require unique validated row count to equal the source total.
5. Admit only source-proven exact-date empty batches.
6. Make live/load probes request the full 200-row transport bound.
7. Run format, crate tests, Clippy and an exact historical-date live probe.
8. Publish a scoped upstream commit for downstream revision pinning.
