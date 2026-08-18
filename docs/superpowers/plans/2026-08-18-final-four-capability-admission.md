# Final four capability admission plan

1. Add deterministic contracts for single-response auction observations,
   complete single-response market breadth and bounded ranking snapshots.
2. Replace the two-call auction and incomplete breadth templates with the exact
   one-call templates proven on 2026-08-18.
3. Add the versioned 2026 offline CFFEX delivery schedule and keep the plaintext
   notice reader diagnostic-only.
4. Register the four exact formal gRPC handlers without an unadmitted opt-in;
   keep wider Level-2, full-market and future-year scopes fail closed.
5. Run contract tests, two live observations and three serial observations for
   each network-backed scope; compare the offline schedule with its fixed
   evidence fixtures.
6. Update BR-034/035/046/051, admission evidence, README and external gRPC docs.
7. Run workspace format, tests, Clippy, compliance, docs and release build;
   deploy the four binaries and verify the remote capability registry and RPCs.
