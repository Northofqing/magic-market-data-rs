# Comprehensive Root README Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the sparse root README with an accurate Chinese-first entry manual for developers and deployment operators.

**Architecture:** Keep the root README as the common orientation and runnable fast path. Summarize capability truth in compact matrices and link protocol fields, performance evidence, deployment detail and rollback policy to their existing canonical documents.

**Tech Stack:** GitHub-flavored Markdown, rolling stable Rust/Cargo commands, repository documentation checks, Bash release tooling.

---

### Task 1: Expand the root README

**Files:**
- Modify: `README.md`
- Reference: `docs/TDX_CAPABILITIES.md`
- Reference: `docs/integrations/tencent-web.md`
- Reference: `docs/integrations/eastmoney-emquant.md`
- Reference: `docs/MULTI_PROVIDER_ROUTING.md`
- Reference: `docs/DEPLOYMENT.md`

- [x] **Step 1: Confirm the sparse baseline**

Run:

```bash
wc -l README.md
```

Expected: fewer than 50 lines and no quick-start, capability matrix, live-probe,
router-use, packaging or security sections.

- [x] **Step 2: Replace the README with the approved information structure**

Write these exact top-level sections in this order:

```markdown
# magic-market-data-rs
## 项目定位
## 工作区结构
## 统一数据契约与证据
## Provider 能力矩阵
## 快速开始
## 真实数据探针
## 多数据源路由
## 构建发布与部署
## 安全与合规边界
## 当前验收状态
## 文档索引
## 上游与许可证
```

The opening must state that the repository is a Rust library workspace plus
read-only diagnostics, not a daemon, database, HTTP API, trading client or
implicit cache/fallback service.

- [x] **Step 3: Add the exact capability truth**

The provider matrix must distinguish these facts:

```text
TDX: live-verified Quote, 12 K-line categories, books, current/history minute
and trades, Shanghai/Shenzhen securities, finance, XDXR, blocks, funds and F10;
normalized money flow and auction unsupported; Quote source time unverified.

Tencent: supplemental public-web source; live-verified Shanghai/Shenzhen/Beijing
Quote/books, bounded K-line/minute boundaries, current Shanghai/Shenzhen trades
and partial metadata; no finance/actions/blocks/money-flow/auction and no SLA.

EMQuant: Rust/bridge mapping implemented for Quote, bars, minute bars,
order book and daily money flow; current device activation refreshed but SDK
login still returns 10001003, so no family is labelled live-accepted; trades,
auction and metadata remain unsupported/unverified.
```

Explain that every normalized record preserves `ProviderId`, `source_at` when
proved, `observed_at`, `batch_id`, `DataStatus` and batch quality issues.

- [x] **Step 4: Add executable setup, probe, router and release commands**

Include these runnable command families:

```bash
rustup toolchain install stable --profile minimal --component rustfmt --component clippy
cargo fetch --locked
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings

cargo run -p magic-tdx-rs --example live_probe --release
cargo run -p magic-tencent-rs --example live_probe --release --locked --offline
cargo run -p magic-tencent-rs --example load_probe --release --locked --offline
cargo run -p magic-market-router --example live_probe --release --locked --offline
cargo run -p magic-emquant-rs --example live_probe --release

bash tools/release/preflight.sh
bash tools/release/package.sh
```

Document the bounded Tencent load variables, EMQuant bridge activation boundary,
probe nonzero-exit semantics, TDX-to-Tencent strict router behavior and release
SHA verification. Include a compact compiling-style `QuoteRouter` registration
example copied from the existing router contract.

- [x] **Step 5: Add deployment, security and documentation navigation**

Summarize platform/network requirements without duplicating full rollback
instructions. Explicitly prohibit storing or packaging credentials, phone
numbers, cookies, `userInfo`, vendor libraries and private login traffic.
Link every canonical document with repository-relative Markdown links.

### Task 2: Record and verify the documentation change

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `.planning/2026-07-23-readme-expansion/task_plan.md`
- Modify: `.planning/2026-07-23-readme-expansion/progress.md`
- Test: `README.md`

- [x] **Step 1: Record the README expansion in the unreleased changelog**

Add one bullet stating that the root README now provides capability truth,
quick start, real probes, routing, release/deployment and security navigation.

- [x] **Step 2: Verify required headings and claims**

Run:

```bash
rg -n '^## (项目定位|工作区结构|统一数据契约与证据|Provider 能力矩阵|快速开始|真实数据探针|多数据源路由|构建发布与部署|安全与合规边界|当前验收状态|文档索引|上游与许可证)$' README.md
rg -n '10001003|source_at|magic-tencent-load-probe|tools/release/package.sh' README.md
```

Expected: all twelve headings and all four operational markers are present.

- [x] **Step 3: Run documentation and release gates**

Run:

```bash
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
git diff --check
bash tools/release/preflight.sh
```

Expected: every command exits zero.

- [x] **Step 4: Self-review capability claims**

Compare each provider row against its canonical document and confirm:

```text
No EMQuant family is called live-passed.
Tencent is not called a production-SLA source.
TDX Quote does not claim verified source time.
Unsupported fields are not represented as zero or successful empty data.
```

- [x] **Step 5: Commit the implementation**

Run:

```bash
git add README.md CHANGELOG.md \
  .planning/2026-07-23-readme-expansion/task_plan.md \
  .planning/2026-07-23-readme-expansion/progress.md
git commit -m "docs: expand project readme"
```

Expected: only the intended tracked documentation changes are committed; the
user's untracked integration requirements document remains untracked.

### Task 3: Package and deliver the final revision

**Files:**
- Generate: `target/dist/GIT_SHA/`
- Verify: `target/dist/GIT_SHA/SHA256SUMS`

- [x] **Step 1: Generate the clean final package**

Run:

```bash
bash tools/release/package.sh
```

Expected: five uniquely named probe binaries plus tracked documentation,
licenses, revision/toolchain metadata and `SHA256SUMS`.

- [x] **Step 2: Verify the package**

Run inside `target/dist/GIT_SHA`:

```bash
shasum -a 256 -c SHA256SUMS
```

Expected: every entry prints `OK`; the package contains no `userInfo`, dynamic
vendor library or `ServerList.json.e`.

- [x] **Step 3: Push and verify the remote**

Run:

```bash
git push
git rev-parse HEAD
git rev-parse '@{u}'
git status --short
```

Expected: local and upstream revisions match exactly, and the only remaining
status entry is the user's untracked
`docs/integrations/stock-analysis-market-data-requirements.md`.
