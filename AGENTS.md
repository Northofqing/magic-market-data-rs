# Engineering rules

Start every task with the `autonomous-long-task` skill. Project startup is not
complete until it is in use: it keeps routine work — reading, editing, running
the documented checks, diagnosing a failure and retrying with new evidence —
moving without a confirmation prompt, and reserves questions for material
decisions and authority boundaries. It does not widen approval policy,
credentials, or external authority; pushing, deploying, and deleting still ask.

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
