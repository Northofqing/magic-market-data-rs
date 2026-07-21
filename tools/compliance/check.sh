#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd); cd "$repo_root"
required=(AGENTS.md Cargo.toml Cargo.lock LICENSE-APACHE LICENSE-MIT LICENSES/tdxrs-MIT.txt docs/ENGINEERING_RULES.md docs/business_rules.md provenance/upstream-files.toml)
for path in "${required[@]}"; do test -s "$path" || { echo "missing required file: $path" >&2; exit 1; }; done
rg -q '^members = \["crates/magic-market-core", "crates/magic-tdx-rs"\]$' Cargo.toml
if rg -n 'stock_analysis' crates/*/Cargo.toml; then exit 1; fi
if rg -n '(todo!\(|unimplemented!\(|panic!\(|unwrap\(|expect\()' crates/*/src --glob '*.rs'; then exit 1; fi
rg -q '^## BR-001 ' docs/business_rules.md; rg -q '^## Gate D ' docs/ENGINEERING_RULES.md
