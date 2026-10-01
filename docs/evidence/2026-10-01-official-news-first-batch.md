# 官方原文与公告候选：第一批实施证据

日期：2026-10-01。范围来自用户确认的先做国内官方原文与公司公告深化。
设计见 [Gate A](../superpowers/specs/2026-10-01-official-news-and-disclosure-design.md)，
正式合同见[国内官方发布](../integrations/official-domestic-publications.md)。

本文保存首次四源准入时的证据快照。后续来源状态、较窄栏目与可见日期合同以
[追加准入证据](2026-10-01-official-news-admission-followup.md)为准；下方原始失败
与当时的验证计数保留，不作为当前所有来源的状态说明。

## 交付范围

新增 `magic-official-news-rs`，统计局、央行、发改委、商务部各自独立准入。
财政部、工信部、海关、能源局、证监会保持未准入，正式方法先于 I/O 拒绝。
只读固定栏目当前一页和同域 HTML 原文；日期精度、实际响应 URL、观察时间、响应
SHA-256 分开保留。没有接入 Service/Router/gRPC、历史归档、全文搜索或自动调度。

原有 Cninfo 标题候选从五类扩展到十四类，包括增持、发行融资、控制权变更、
协议转让、质押/解除、冻结/解冻、回购、激励和员工持股。混合或否定标题不推断
完成事件；计划、批准与实施保留原始标题与 canonical payload。巨潮覆盖文档从
旧的 100 条说法纠正到现有有界 300 条实现，仍不声称完整市场日覆盖。

## 实测方法

每源两轮正式 `live_probe SOURCE`；每轮完整验证列表并读取前两篇正文，比较标题和
日期。一轮 `live_probe SOURCE --serial-load` 在同一 client 上做三次串行列表读取。
四个不同主机可以并行，同一主机请求保持客户端一秒门限；没有使用登录、Cookie、
验证码、浏览器专用头、代理、重定向、HTTP 升级或 TLS 例外。

原始 HTML/JSON 诊断样本和逐轮读取输出保留在本机临时 probe 目录。仓库只保留
验证元数据和响应哈希，不复制整篇新闻正文。正式实测摘要与逐轮记录见下方。

## 排错和依赖检查

1. 初始构建曾在源文件尚未写齐时进行，报告缺失动态列表辅助函数；确认文件写齐后
   新构建通过。后续编译和源文件修改按顺序进行。
2. `scraper` 实验引入四个 MPL-2.0 包，既有许可证检查明确失败；已移除该方案，
   使用 MIT 的 `tl` 与 `html-escape`，没有放宽 `deny.toml`。`tl` 不实现父子 CSS
   匹配的部分由封闭模板适配器完成；回归曾发现列表/正文漏选并据此修复。
3. 全量审计发现既有共享传输 `rustls =0.23.42` 命中
   [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285)。依据
   [维护者修复公告](https://github.com/rustls/rustls/security/advisories/GHSA-2mjx-qc3c-rqvc)
   更新固定版本到 `=0.23.45`，保持功能开关和传输策略；相关 `rustls-webpki`
   锁定解析到 `0.103.15`。修复后的正式构建重新执行四源探针。
4. HTTP 传输登记初版给 shared/infrastructure 行填写了说明文本，机械检查要求这些
   行的 reason 为标准占位 `-`，因此首次检查失败。已恢复规定格式，详细理由保留在
   Gate A 与本证据中；复查通过 28 个传输登记和 60 个能力登记。

解析器适配编译还报告过 `InlineVec` 不支持 `contains` / 双向 `rev`；改为其公开
迭代接口与有界局部向量后重建。未通过的构建没有用于正式准入计数。

## 仍未解决的来源问题

财政部混合 HTTP 子站链接；工信部 24 条中 23 条明确 HTTP；海关独立匿名探针
遇到默认 TLS 证书链错误且 Rust 模板未验证；能源局 HTTPS 动态列表为空，必要脚本
为 HTTP；证监会列表与正文 PubDate 不同，正文日期字段与页面生成时间重合。
这些失败没有被隐藏成空新闻，也没有通过改写 URL、忽略字段或绕过 TLS 消除。
证监会诊断列表保留自己的时间标签，正文诊断在网络前返回 `Unsupported`。
详见[独立入口核验](2026-10-01-domestic-official-listing-endpoints.md)。

## Gate C

在最终标题修复后执行：

| 检查 | 结果 |
|---|---|
| `cargo test --workspace --all-targets --locked --offline` | 1963 passed、0 failed、2 ignored；249 个测试目标摘要 |
| 新官方原文 crate 单元测试（在工作区运行中） | 11 passed；覆盖精度、正文、模板、JSON 分页、未准入、URL/媒体/响应边界和 clone 限速 |
| content discovery 单元测试（在工作区运行中） | 7 passed；含保留原始 payload 和否定/分红登记日回归 |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | 通过，无警告 |
| `cargo fmt --all -- --check` | 通过 |
| `bash tools/compliance/check.sh` | 通过；60 个能力、28 个传输登记，以及 TDX/gRPC 边界 |
| `bash tools/docs/check_links.sh` | 通过；最终证据补写后复查 |
| `cargo deny check --hide-inclusion-graph` | advisories/bans/licenses/sources 全部通过；保留五项既有重复版本警告 |
| bench compare 工具单元测试 | 1 passed |
| coverage 工具单元测试 | 13 passed；这是工具回归，不是本批实际代码行覆盖率声明 |
| `cargo doc --workspace --no-deps --locked --offline` | 通过，生成 41 个 crate 的文档入口 |

未进行发布、部署或推送；结果保留在工作区，仓库原有未提交变更继续保留。

最终语义核对另外补了公告标题边界：`不会导致实际控制人变更`不应成为控制权变更
候选，`利润分配股权登记日`中的字符子串`配股`不应成为发行候选。增加了独立的
否定及分红登记日回归样例，同时保留真正配股发行标题的正例。

## 修复后正式读取记录

四源正式读取均成功；使用 `rustls =0.23.45` 的最终构建。
完整元数据、两轮各两篇正文的 URL/日期标签/精度/文本长度及哈希保留于
[正式实测摘要](2026-10-01-official-news-live-summary.json)。每源独立的计数为
两轮 live probe、三次 serial load；每轮 live 含三次 HTTP 读取。

| Source | 轮次 | 源行数 | 列表 observed_at (UTC) | 列表响应 SHA-256 |
|---|---|---:|---|---|
| Nbs | probe-1 | 15 | 2026-10-01T08:12:59.6640341Z | `40064f27414db06a7d2eaf62c5fac94d177ed2707db9827b9a679642fe772fcd` |
| Nbs | probe-2 | 15 | 2026-10-01T08:13:03.5451427Z | `40064f27414db06a7d2eaf62c5fac94d177ed2707db9827b9a679642fe772fcd` |
| Nbs | serial-load-1 | 15 | 2026-10-01T08:13:06.8948909Z | `40064f27414db06a7d2eaf62c5fac94d177ed2707db9827b9a679642fe772fcd` |
| Nbs | serial-load-2 | 15 | 2026-10-01T08:13:07.5240523Z | `40064f27414db06a7d2eaf62c5fac94d177ed2707db9827b9a679642fe772fcd` |
| Nbs | serial-load-3 | 15 | 2026-10-01T08:13:08.5438165Z | `40064f27414db06a7d2eaf62c5fac94d177ed2707db9827b9a679642fe772fcd` |
| Pbc | probe-1 | 15 | 2026-10-01T08:12:59.3704745Z | `7da4b61e81c171531f107fa20ca56cac9b66ab192956f616fabeb0e24c805e18` |
| Pbc | probe-2 | 15 | 2026-10-01T08:13:03.0028371Z | `7da4b61e81c171531f107fa20ca56cac9b66ab192956f616fabeb0e24c805e18` |
| Pbc | serial-load-1 | 15 | 2026-10-01T08:13:05.8942835Z | `7da4b61e81c171531f107fa20ca56cac9b66ab192956f616fabeb0e24c805e18` |
| Pbc | serial-load-2 | 15 | 2026-10-01T08:13:06.6941761Z | `7da4b61e81c171531f107fa20ca56cac9b66ab192956f616fabeb0e24c805e18` |
| Pbc | serial-load-3 | 15 | 2026-10-01T08:13:07.6939596Z | `7da4b61e81c171531f107fa20ca56cac9b66ab192956f616fabeb0e24c805e18` |
| Ndrc | probe-1 | 25 | 2026-10-01T08:12:59.3715365Z | `6482ab4e14cd2e59449ecfe6ee566764fc0a7cad5aa7b7af7ba97bd73829e1e4` |
| Ndrc | probe-2 | 25 | 2026-10-01T08:13:03.2455825Z | `6482ab4e14cd2e59449ecfe6ee566764fc0a7cad5aa7b7af7ba97bd73829e1e4` |
| Ndrc | serial-load-1 | 25 | 2026-10-01T08:13:05.8931846Z | `6482ab4e14cd2e59449ecfe6ee566764fc0a7cad5aa7b7af7ba97bd73829e1e4` |
| Ndrc | serial-load-2 | 25 | 2026-10-01T08:13:06.6711559Z | `6482ab4e14cd2e59449ecfe6ee566764fc0a7cad5aa7b7af7ba97bd73829e1e4` |
| Ndrc | serial-load-3 | 25 | 2026-10-01T08:13:07.6729256Z | `6482ab4e14cd2e59449ecfe6ee566764fc0a7cad5aa7b7af7ba97bd73829e1e4` |
| Mofcom | probe-1 | 15 | 2026-10-01T08:12:59.4932527Z | `368a5d42d16955abd0ef9acd9138362e26563336fe5eebed4f5512141a56838b` |
| Mofcom | probe-2 | 15 | 2026-10-01T08:13:03.0940902Z | `61242c961d78cfd6f55177a7d1f21454d46ef6bd433a04ed5a0a819096ca142f` |
| Mofcom | serial-load-1 | 15 | 2026-10-01T08:13:05.9424711Z | `0e49e3517977ca13cd8a2bd8282f5c2472bd78ff13679897ba893be5575b981f` |
| Mofcom | serial-load-2 | 15 | 2026-10-01T08:13:06.644563Z | `ee3c25857b38759a1e31e51e7175b93f9e26ada0016f8f5706da438880af372b` |
| Mofcom | serial-load-3 | 15 | 2026-10-01T08:13:07.6171618Z | `e9022a7d1ab1ba7307a9abd8a775efaac761a887a728b7520855639262758904` |
