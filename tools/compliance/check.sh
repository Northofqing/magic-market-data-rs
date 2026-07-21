#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd); cd "$repo_root"
required=(AGENTS.md Cargo.toml Cargo.lock LICENSE-APACHE LICENSE-MIT LICENSES/tdxrs-MIT.txt docs/ENGINEERING_RULES.md docs/business_rules.md)
for path in "${required[@]}"; do test -s "$path" || { echo "missing required file: $path" >&2; exit 1; }; done
rg -q '^members = \["crates/magic-market-core", "crates/magic-tdx-rs"\]$' Cargo.toml
if rg -n 'stock_analysis' crates/*/Cargo.toml; then exit 1; fi
# Imported upstream modules retain documented/test-only unwrap examples; runtime
# hardening is tracked separately from this structural compliance gate.
rg -q '^## BR-001 ' docs/business_rules.md; rg -q '^## Gate D ' docs/ENGINEERING_RULES.md
