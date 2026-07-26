# CFFEX Capability Remediation Design

**Date:** 2026-07-26
**Status:** Gate A approved for the unified-data release branch
**Rules:** BR-009, BR-018, BR-021

## Objective

Correct two unsupported claims in the CFFEX futures-delivery contract:

1. a deterministic parser is not a production-admitted capability when the
   bounded live probe cannot establish TLS to the official notice directory;
2. delivery-settlement-price wording proves the event and settlement price,
   but does not independently prove the settlement method.

No formula, cached calendar, alternate transport, or downstream fallback is
introduced.

## Data flow

The public production trait remains the stable provider-neutral seam:

```text
FuturesDeliveryCalendar::futures_delivery_calendar
  -> typed Unsupported while capability=false
```

Admission diagnostics use a separate explicit path:

```text
CffexClient::probe_futures_delivery_calendar
  -> exact HTTPS allowlist
  -> bounded notice-list scan
  -> exact same-host detail
  -> strict IF/IH/IC/IM/date/settlement-price validation
  -> DataBatch<FuturesDeliveryEvent(method=NotProvided)>
```

The diagnostic path executes the production parser and provenance checks. It
does not advertise availability and is never invoked implicitly by the
production trait.

## Failure modes

- TLS, timeout, HTTP, MIME, final-URL, pagination, schema, identity, date,
  cardinality, provenance, or quality failure remains a typed error.
- A missing official notice remains a typed `Incomplete` error; it is not converted to a
  calculated third Friday.
- Notice publication date is the provenance `source_at`; delivery date remains
  an event field and is never substituted for source publication time.
- Missing last-trading-date evidence remains `None`; it is not copied from the
  delivery date.
- Missing settlement-method evidence remains `NotProvided`; it is not converted
  to `Cash`.
- Capability stays false until a current bounded live probe succeeds and its
  evidence is reviewed.

## Old module relation

| module/path | decision | reason |
| --- | --- | --- |
| existing CFFEX strict parser | adopt | deterministic validation and evidence remain valid |
| production trait calling the unadmitted parser | reject | contradicted BR-009/BR-021 live admission |
| `FuturesDeliveryMethod::Cash` inferred from settlement price | reject | source notice does not prove the method |
| formula/calendar fallback | reject | prohibited by BR-018 |

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --locked -- --test-threads=1
bash tools/compliance/check.sh
bash tools/docs/check_links.sh
git diff --check
cargo llvm-cov --workspace --all-features --json \
  --output-path target/coverage/coverage.json -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
MAGIC_EXCHANGE_LIVE_OPERATION=cffex-delivery \
  cargo run -p magic-exchange-rs --example live_probe --release --locked
```

The live command may remain a truthful typed transport failure. Such a result
does not pass capability admission.
Diagnostic success emits `diagnostic_probe_status=passed` together with
`admission_state=diagnostic_complete_unadmitted`; it never emits the production
`live_probe_status=passed` marker.

## Rollback

Revert the focused remediation commit. A rollback may restore code only after
new reviewed live evidence proves both availability and method; it must not
restore formula inference, fallback transport, or an unsupported capability
claim.
