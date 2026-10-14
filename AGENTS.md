# Engineering rules

Run formatting, tests, Clippy, compliance, and documentation checks before
release. Preserve explicit failures and provenance; do not add downstream path
dependencies. Changes follow Gates A through D and registered business rules.

Before changing contracts or architecture, read
[`docs/ENGINEERING_RULES.md`](docs/ENGINEERING_RULES.md) and
[`docs/business_rules.md`](docs/business_rules.md). Provider admission evidence
is governed by
[`docs/integrations/admissions.tsv`](docs/integrations/admissions.tsv).

HTTP dependencies are governed by
[`docs/integrations/http-transports.tsv`](docs/integrations/http-transports.tsv).
Do not add or widen a provider-local HTTP/TLS dependency, bypass endpoint
allowlists, or weaken timeout/body/redirect policy without an approved Gate A
design and matching registry update. HTTP Provider calls are currently blocking;
follow [`docs/integrations/async-blocking.md`](docs/integrations/async-blocking.md)
when integrating them with an async runtime.
