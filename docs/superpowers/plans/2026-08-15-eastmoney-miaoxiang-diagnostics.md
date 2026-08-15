# Eastmoney Miaoxiang diagnostic implementation plan

1. Register BR-046, the exact authenticated endpoint and blocked admission rows.
2. Add a redacted-key client, fixed query templates and strict response parser to
   `magic-eastmoney-rs` without changing the legacy public client.
3. Add deterministic fixtures for auth redaction, endpoint/query bounds, outer and
   inner status, identity/date/unit/cardinality checks and partial-field retention.
4. Register opt-in gRPC diagnostic handlers for daily fund flow, opening auction
   and partial market breadth. Default calls continue to fail before provider I/O.
5. Update external integration documentation and run formatting, tests, Clippy,
   docs, compliance, dependency and diff checks.
