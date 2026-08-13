#!/usr/bin/env bash
set -euo pipefail
cd -- "${BASH_SOURCE[0]%/*}/../.."
repo_root=$PWD
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
  docs/integrations/README.md
  docs/integrations/admissions.tsv
  docs/integrations/async-blocking.md
  docs/integrations/http-transports.tsv
  docs/integrations/tdx-local-terminal.md
  docs/integrations/tdx-local-terminal-compatibility.tsv
  docs/integrations/sina-web.md
  docs/integrations/eastmoney-web.md
  docs/integrations/cninfo-web.md
  docs/integrations/tonghuashun-web.md
  docs/integrations/cls-web.md
  docs/integrations/jin10-web.md
  docs/integrations/thepaper-web.md
  docs/integrations/yonhap-rss.md
  docs/integrations/wallstreetcn-rss.md
  docs/integrations/baidu-web.md
  docs/integrations/iwencai-api.md
  docs/integrations/exchange-official.md
  docs/integrations/level2-auction.md
  docs/integrations/broker-account-boundary.md
  docs/integrations/gov-policy.md
  docs/integrations/nbs-official.md
  docs/integrations/pbc-official.md
  docs/integrations/cfets-official.md
  docs/integrations/fred-api.md
  docs/integrations/imf-datamapper.md
  docs/integrations/worldbank-indicators.md
  docs/integrations/sec-edgar.md
  docs/integrations/xinhua-finance.md
  docs/integrations/yicai-news.md
  docs/integrations/securities-times.md
  crates/magic-market-transport/Cargo.toml
  crates/magic-nbs-rs/Cargo.toml
  crates/magic-pbc-rs/Cargo.toml
  crates/magic-cfets-rs/Cargo.toml
  crates/magic-fred-rs/Cargo.toml
  crates/magic-imf-rs/Cargo.toml
  crates/magic-worldbank-rs/Cargo.toml
  crates/magic-sec-rs/Cargo.toml
  crates/magic-xinhua-rs/Cargo.toml
  crates/magic-yicai-rs/Cargo.toml
  crates/magic-stcn-rs/Cargo.toml
  crates/magic-market-router/Cargo.toml
  crates/magic-market-composition/Cargo.toml
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
  crates/magic-wallstreetcn-rs/Cargo.toml
  crates/magic-tdx-local-rs/Cargo.toml
  crates/magic-tdx-native-bridge/Cargo.toml
  crates/magic-market-monitor/Cargo.toml
  crates/magic-market-monitor-server/Cargo.toml
  crates/magic-market-grpc-contracts/Cargo.toml
  crates/magic-market-service/Cargo.toml
  crates/magic-market-grpc-server/Cargo.toml
  crates/magic-market-tdx-agent/Cargo.toml
  crates/magic-baidu-rs/Cargo.toml
  crates/magic-iwencai-rs/Cargo.toml
  crates/magic-exchange-rs/Cargo.toml
  crates/magic-gov-rs/Cargo.toml
)
for required_file in "${required[@]}"; do
  test -s "$required_file" || {
    echo "missing required file: $required_file" >&2
    exit 1
  }
done
python_cmd=(python3)
if [[ "${OS:-}" == Windows_NT ]] && command -v py >/dev/null 2>&1; then
  python_cmd=(py -3)
fi
"${python_cmd[@]}" tools/compliance/check_admissions.py
"${python_cmd[@]}" tools/compliance/check_http_transports.py
"${python_cmd[@]}" tools/compliance/check_tdx_local_compatibility.py
"${python_cmd[@]}" tools/compliance/check_tdx_native_boundary.py
"${python_cmd[@]}" tools/compliance/check_grpc_services.py

for toolchain_file in rust-toolchain rust-toolchain.toml; do
  if [[ -e "$toolchain_file" ]]; then
    printf 'fixed repository toolchain selector is not allowed: %s\n' \
      "$toolchain_file" >&2
    exit 1
  fi
done
if rg -n '^[[:space:]]*rust-version[[:space:]]*=' \
  Cargo.toml crates/*/Cargo.toml; then
  echo "Cargo manifests must not declare a fixed Rust version" >&2
  exit 1
fi
if rg -n 'dtolnay/rust-toolchain@[0-9]+([.][0-9]+)*' .github/workflows; then
  echo "active workflows must use the stable Rust toolchain" >&2
  exit 1
fi
if rg -n 'RUSTUP_TOOLCHAIN=[0-9]+([.][0-9]+)*|cargo[[:space:]]+[+][0-9]+([.][0-9]+)*' \
  .github/workflows tools; then
  echo "active build and release tooling must not select a numeric Rust version" >&2
  exit 1
fi

workspace_members=(
  crates/magic-market-core
  crates/magic-market-router
  crates/magic-market-composition
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
  crates/magic-wallstreetcn-rs
  crates/magic-tdx-local-rs
  crates/magic-tdx-native-bridge
  crates/magic-market-monitor
  crates/magic-market-monitor-server
  crates/magic-market-grpc-contracts
  crates/magic-market-service
  crates/magic-market-grpc-server
  crates/magic-market-tdx-agent
  crates/magic-market-transport
  crates/magic-nbs-rs
  crates/magic-pbc-rs
  crates/magic-cfets-rs
  crates/magic-fred-rs
  crates/magic-imf-rs
  crates/magic-worldbank-rs
  crates/magic-sec-rs
  crates/magic-xinhua-rs
  crates/magic-yicai-rs
  crates/magic-stcn-rs
)
for member in "${workspace_members[@]}"; do
  sed -n '/^members = \[/,/^\]/p' Cargo.toml | rg -Fq "\"$member\"" || {
    echo "missing workspace member: $member" >&2
    exit 1
  }
done
expected_workspace_crate_version=0.2.0
while IFS= read -r manifest; do
  if ! awk -v expected="$expected_workspace_crate_version" -v manifest="$manifest" '
    /^\[package\]$/ { in_package=1; next }
    /^\[/ && in_package { exit }
    in_package && /^version = "/ {
      version=$0
      gsub(/^version = "|".*$/, "", version)
      found=1
      if (version != expected) {
        printf "workspace crate version mismatch: %s expected=%s actual=%s\n", \
          manifest, expected, version > "/dev/stderr"
        exit 1
      }
      exit
    }
    END {
      if (!found) {
        printf "workspace crate version missing: %s\n", manifest > "/dev/stderr"
        exit 1
      }
    }
  ' "$manifest"; then
    exit 1
  fi
done < <(find crates -mindepth 2 -maxdepth 2 -name Cargo.toml -print | LC_ALL=C sort)

if rg -n 'stock_analysis' crates/*/Cargo.toml; then exit 1; fi
if sed -n '/^\[dependencies\]/,/^\[/p' crates/magic-market-router/Cargo.toml | \
  rg -q 'magic-(tdx|tencent|sina|emquant|eastmoney|cninfo|ths|cls|jin10|thepaper|yonhap|wallstreetcn|baidu|iwencai|exchange|nbs|pbc|cfets|fred|imf|worldbank|sec|xinhua|yicai|stcn)-rs'; then
  echo "router production dependencies must remain provider-neutral" >&2
  exit 1
fi
# Imported upstream modules retain documented/test-only unwrap examples; runtime
# hardening is tracked separately from this structural compliance gate.
for number in {1..44}; do
  printf -v rule_id 'BR-%03d' "$number"
  rg -q "^## $rule_id " docs/business_rules.md || {
    echo "missing registered business rule: $rule_id" >&2
    exit 1
  }
done
rg -q '^## Gate D ' docs/ENGINEERING_RULES.md
