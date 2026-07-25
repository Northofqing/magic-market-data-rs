#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd); cd "$repo_root"
required=(
  AGENTS.md
  Cargo.toml
  Cargo.lock
  LICENSE-APACHE
  LICENSE-MIT
  LICENSES/tdxrs-MIT.txt
  docs/ENGINEERING_RULES.md
  docs/business_rules.md
  docs/DEPLOYMENT.md
  docs/MULTI_PROVIDER_ROUTING.md
  docs/integrations/tencent-web.md
  docs/integrations/sina-web.md
  docs/integrations/eastmoney-web.md
  docs/integrations/cninfo-web.md
  docs/integrations/tonghuashun-web.md
  docs/integrations/cls-web.md
  docs/integrations/jin10-web.md
  docs/integrations/thepaper-web.md
  docs/integrations/yonhap-rss.md
  docs/integrations/baidu-web.md
  docs/integrations/iwencai-api.md
  docs/integrations/exchange-official.md
  docs/integrations/gov-policy.md
  crates/magic-market-router/Cargo.toml
  crates/magic-market-analysis/Cargo.toml
  crates/magic-tencent-rs/Cargo.toml
  crates/magic-sina-rs/Cargo.toml
  crates/magic-eastmoney-rs/Cargo.toml
  crates/magic-cninfo-rs/Cargo.toml
  crates/magic-ths-rs/Cargo.toml
  crates/magic-cls-rs/Cargo.toml
  crates/magic-jin10-rs/Cargo.toml
  crates/magic-thepaper-rs/Cargo.toml
  crates/magic-yonhap-rs/Cargo.toml
  crates/magic-baidu-rs/Cargo.toml
  crates/magic-iwencai-rs/Cargo.toml
  crates/magic-exchange-rs/Cargo.toml
  crates/magic-gov-rs/Cargo.toml
)
for path in "${required[@]}"; do test -s "$path" || { echo "missing required file: $path" >&2; exit 1; }; done
workspace_members=(
  crates/magic-market-core
  crates/magic-market-router
  crates/magic-tdx-rs
  crates/magic-emquant-rs
  crates/magic-tencent-rs
  crates/magic-sina-rs
  crates/magic-market-analysis
  crates/magic-eastmoney-rs
  crates/magic-cninfo-rs
  crates/magic-ths-rs
  crates/magic-cls-rs
  crates/magic-jin10-rs
  crates/magic-thepaper-rs
  crates/magic-baidu-rs
  crates/magic-iwencai-rs
  crates/magic-exchange-rs
  crates/magic-gov-rs
  crates/magic-yonhap-rs
)
workspace_manifest_members=$(sed -n '/^members = \[/,/^\]/p' Cargo.toml)
for member in "${workspace_members[@]}"; do
  rg -Fq "\"$member\"" <<<"$workspace_manifest_members" || {
    echo "missing workspace member: $member" >&2
    exit 1
  }
done
if rg -n 'stock_analysis' crates/*/Cargo.toml; then exit 1; fi
router_dependencies=$(sed -n '/^\[dependencies\]/,/^\[/p' crates/magic-market-router/Cargo.toml)
if rg -q 'magic-(tdx|tencent|sina|emquant|eastmoney|cninfo|ths|cls|jin10|thepaper|yonhap|baidu|iwencai|exchange)-rs' <<<"$router_dependencies"; then
  echo "router production dependencies must remain provider-neutral" >&2
  exit 1
fi
# Imported upstream modules retain documented/test-only unwrap examples; runtime
# hardening is tracked separately from this structural compliance gate.
rg -q '^## BR-001 ' docs/business_rules.md
rg -q '^## BR-020 ' docs/business_rules.md
rg -q '^## BR-021 ' docs/business_rules.md
rg -q '^## Gate D ' docs/ENGINEERING_RULES.md
