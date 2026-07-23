# Root README expansion design

## Goal

Make the repository root README sufficient for a new developer to understand
what the workspace provides, build it deterministically, run real probes, use
the failover router and locate deployment details without overstating any data
source.

## Audience

The primary readers are Rust application developers integrating normalized
market data and operators validating a deployment. Provider specialists can
follow links to the existing detailed contracts.

## Considered approaches

1. Add only installation and quick-start commands. This stays short but leaves
   the current capability and deployment questions unanswered.
2. Copy all provider manuals into the README. This is exhaustive but creates
   conflicting duplicated truth and an unmaintainable front page.
3. Build an operator-first entry manual with accurate summary matrices,
   executable commands and links to canonical details. This is the selected
   approach.

## Information architecture

The README will use this order:

1. project purpose, current maturity and explicit non-goals;
2. workspace/crate map and normalized data/evidence model;
3. provider capability matrix with live-verification boundaries;
4. fast deterministic setup and test commands;
5. real-network probes, environment variables and expected failure semantics;
6. provider-neutral routing model and a compact Rust example;
7. release packaging, platform/network requirements and deployment gates;
8. security, licensing, upstream provenance and documentation index.

The first screen must tell the reader that this is a library workspace plus
diagnostic probes, not a database, daemon, HTTP API, trading client or hidden
fallback service.

## Capability language

Claims use three distinct states:

- implemented and live-verified;
- implemented but awaiting account/source verification;
- unsupported with explicit typed errors.

TDX and Tencent live evidence may be summarized with its recorded date.
EMQuant must state that device activation succeeded but official SDK login
still returns `10001003/EQERR_NO_ACCESS`; it cannot be presented as live data.
Tencent must be labelled a supplemental public-web source without a production
SLA. TDX Quote source time must not be represented as verified.

## Commands and examples

Commands will use the pinned Rust stable toolchain, `--locked` and offline mode
where the existing workflow supports it. The README will include:

- toolchain installation and dependency fetch;
- deterministic workspace checks;
- TDX, Tencent, EMQuant and router live probes;
- bounded Tencent load-probe variables;
- release preflight, packaging and SHA-256 verification;
- a compact `QuoteRouter` registration example that preserves explicit error
  classification.

No command will embed credentials, phone numbers, activation tokens or vendor
SDK content.

## Duplication boundary

The README owns orientation, current status, the common fast path and links.
Provider field positions, protocol-specific limitations, EMQuant file layout,
full deployment rollback and performance raw evidence remain canonical in the
existing detailed documents.

## Verification

The change must pass:

```text
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
git diff --check
```

A final manual audit will compare every capability row and live-status claim
against the provider documents. Packaging will be regenerated only after the
tracked worktree is committed, and every file in `SHA256SUMS` must verify.
