# magic-market-data-rs

面向中国证券市场的只读 Rust 数据基础设施。项目把不同数据源转换为统一的强类型合同，
通过 Rust API 和版本化 gRPC 对外提供行情、资讯、宏观数据、市场事件及 TDX 本地终端
观测。

项目优先保证数据身份、单位、时间和来源证据正确。缺失、过期、部分响应或未通过验收的
能力会明确失败或标记为 `UNADMITTED`，不会用零值、缓存、另一数据源或推测字段补齐。

## 能做什么

- 获取实时行情、K 线、分时、逐笔、五档、证券元数据、财务与公司行动等标准化数据。
- 接入 TDX、Tencent、Sina、Eastmoney、CNInfo、THS、交易所及多种新闻、宏观数据源。
- 使用 Router 按固定顺序切换数据源，同时保留每次尝试和最终来源。
- 通过 `magic.market.v1` gRPC 查询数据、读取能力状态、订阅事件和执行有界重放。
- 在 Windows 自动发现当前用户会话中的通达信客户端，通过固定本机 TQ-Local 只读接口
  获取价格、累计成交量和累计成交额。
- 通过 gRPC 动态替换 TDX 监控列表，标的格式为
  `EQUITY:SH|SZ|BJ:NNNNNN`。
- 提供价格、成交额、成交量的确定性异动计算和事件合同；只有显式版本化规则产生的
  trigger/rearm `AnomalyEvent` 才是生产事件，预热、冷却和 reset 仍是状态消息。
- 提供指数行情、分时形态、TDX 四族 T0 证据、结果日线和涨停池复盘等组合数据产品；
  盘后资金流可按当前本地观察时刻读取，每项能力独立准入。
- 提供东财单响应的有界市场排名快照，以及妙想单响应的全 A 宽度和开盘竞价窄合同；
  排名不冒充完整市场分页，竞价的 Level-2 专属字段保持空值，妙想能力需要运行时 Key。
- 提供版本化的 2026 CFFEX IF/IH/IC/IM 月度交割日历；正式调用使用仓库内固定表，
  不依赖运行时明文 HTTP，也不会用日期公式扩展到其他年份。
- 通过官方 EMQuant/Choice SDK 提供沪深股票的显式日期范围、未复权完成日线；权限到期、
  当日字段未完成或 SDK 不可用时返回无 records 的类型化失败，不填零、不回退旧数据。
- 通过官方同花顺扶摇 Financial API 提供沪深北股票、标准指数和 ETF 的未复权完成日线，
  以及估值子集、显式日期涨停/跌停/炸板池、当前热股榜、A 股财务三表、现金/送股公司行动
  和字段级可用性明确的证券元数据；Key 缺失、到期或无权限时返回类型化失败，不回退网页源。
- 为每条记录保留 Provider、批次、源时间（源能够证明时）、本地观测时间、质量状态和
  完整性证据。

精确到数据族和 Provider 的当前状态以
[准入注册表](docs/integrations/admissions.tsv) 为准；gRPC 请求与响应见
[外部对接文档](docs/integrations/grpc-external-api.md)。当前 10 条未准入
Provider×operation 路径、已发现的官方接口及显式替代范围见
[未准入路径与显式替代矩阵](docs/integrations/unadmitted-provider-routes.md)。

## gRPC 新闻证据合同

`GlobalNews` 的每条 `records[].data` 都携带自己的 `evidence.provider`、原始
`evidence.source_at`、`evidence.observed_at` 和 `evidence.batch_id`。批次
`QueryResponse.source_at` 只表示最新记录的来源时间，不能用来构造或覆盖逐条 evidence。
记录 evidence 缺失、混批或与 `published_at` 冲突时，整批以类型化
`invalid_evidence` 拒绝，不返回部分成功。

`InstrumentNews` schema v2 使用调用方给出的精确 RFC3339 `captured_through`。服务端先
校验完整 Sina 上游批次，再过滤晚于截止时刻的记录；合法 cutoff-empty 返回
`ADMITTED`、`complete=true` 和空 records，保留真实 `batch_id`/`observed_at`，且不伪造
批次 `source_at`。无法证明的空批次和错误 evidence 仍然 fail-closed。

当前对接合同交付基线为 client-bundle `2026-08-27.3`。该版本将 `T0Evidence`
升级为必须携带调用方精确 `requested_at` 的 v2，并增加运行构建身份与安全、有序的完整
Provider attempts；同时修复 TDX 形成中日线、Sina 个股新闻原始 URL 查询分隔符和东财公开
日级资金流路由（固定使用东财官方 delay 主机以兼容严格 TLS 客户端），并按深交所正式代码
区间接受 CLS 的合法 `sz302132` 关联股票。完整逐条
evidence、空批次和失败分类合同以
[gRPC 外部对接文档](docs/integrations/grpc-external-api.md)为准。bundle 由
[`tools/docs/build_client_bundle.ps1`](tools/docs/build_client_bundle.ps1)生成，并使用
LF 格式的 `manifest.sha256` 做跨平台校验；精确来源提交以 bundle 内
`bundle-metadata.json` 的 `source_commit` 为准。

运行时 stderr 日志统一携带 UTC RFC3339 时间戳。`GetHealth` 还返回源码 revision、协议
descriptor 与当前二进制 SHA-256；`GetHealth` 和
`GetListenerStatus` 通过向后兼容的追加字段提供无高基数标签的聚合 query、并发、Agent、
subscriber 和 replay 指标；成功行情请求不逐条落日志，避免把同步日志 I/O 放入热路径。

## 明确边界

本项目只处理市场数据，不提供：

- 策略回测、参数优化、撮合、滑点、手续费、资金曲线或绩效归因；
- 模拟交易、实盘下单、撤单、账户、资金、持仓或成交回报；
- 数据库、数据湖、长期历史仓库或跨请求缓存；
- 客户端批次审计落库、去重/保留策略、FIFO/T+1 持仓账本或候选扫描解释；
- 对公共网页、厂商服务可用性和数据再分发权的 SLA 承诺；
- 未经证明的字段推导、跨源静默拼接或未授权接口访问。

当前没有完整 Level-2 集合竞价权限、CFETS DR007 机器接口授权和 IMF beta/SDMX
账户合同。完整 Level-2、DR007 和 IMF 合同继续保留为不可用；已有的 CFETS Shibor、
LPR 和官方汇率能力不受 DR007 授权缺口影响。窄版 `Auctions` 只返回同一妙想响应明确
提供的竞价成交量和成交额，匹配价、昨收、未匹配买卖队列、量比和 Provider 源时刻均
保持 `null`，不会从普通行情推导。同花顺扶摇的当前最终竞价诊断另行返回源直接给出的成交价、
昨收、成交量（严格从手换算为股）、成交额和量比；因快照本身未给交易日、逐条源时刻和
方向化未匹配队列，它保持 repository-unadmitted，只能显式 `allow_unadmitted=true` 调用。
同花顺交易日历和竞价短线基准虽然包含日期，但日期不与该快照记录绑定，不能跨响应补证据。

`MarketBreadth` 的上市总数、涨跌平、涨跌停和覆盖率来自同一个妙想响应；这证明采集
原子性，但 Provider 没有给每个字段源时刻，因此 `maximum_source_skew_millis` 保持
`null`。gRPC `MarketRankings` 也只声明一次 Eastmoney HTTPS 响应中的 Top-N 快照，
不声明完整市场分页、截止位并列完整性或市场宽度。

TDX 历史 K 线、分时、逐笔、财务和公司行动可以作为外部回测系统的数据输入，但
“能够读取历史数据”不等于本仓库已经实现回测引擎。

TDX 本地终端集成只访问固定的 `http://127.0.0.1:17709/` 只读方法，不开放该端口，
不加载厂商 DLL，不依赖 Python，也不调用账户或交易接口。当前生产准入覆盖
`Now`、`Volume`、`Amount`、昨收、开高低和带完整 Core 证据的三类异动触发；源时间和
`source_record_count` 仍不可用，预热、冷却和 reset 不冒充异动触发。逐帧字段缺失只会
关闭对应字段的 admission；坏帧或 Monitor 输出关闭会以新 generation 重启，瞬时 gRPC
故障重连，而鉴权、配置和序列冲突仍明确失败。

诊断接口即使成功返回数据，也不会自动成为生产能力。只有完成合同测试、真实探针和
注册表更新的精确范围才能标记为 `ADMITTED`。

## 架构

```text
Provider crates
      │
      ▼
magic-market-core       标准化合同、证据和值对象
      │
      ▼
magic-market-router     质量门、来源时间门、顺序切源
      │
      ▼
magic-market-composition 绑定真实 Provider 与组合数据产品
      │
      ▼
magic-market-service    与传输无关的操作注册和准入门
      │
      ▼
magic-market-grpc-server 认证、限流、并发预算和外部 gRPC
```

TDX 本地监听是独立叶子链路：

```text
TdxW.exe
  └─ fixed loopback HTTP
       └─ magic-market-monitor-server
            └─ magic-market-tdx-agent（只向外连接）
                 └─ magic-market-grpc-server
```

核心 crate 不依赖 Protobuf、gRPC 或具体 Provider。Router 的生产依赖只有 Core；具体
数据源只在 composition 边界组合，避免公共合同反向依赖厂商实现。

主要 crate：

| Crate | 责任 |
| --- | --- |
| `magic-market-core` | Provider 无关的数据合同、证据、状态和值对象 |
| `magic-market-router` | 顺序切源、质量与新鲜度校验、attempt trace |
| `magic-market-composition` | 生产 Provider 和组合数据产品绑定 |
| `magic-market-service` | 操作注册、能力与 admission-before-I/O |
| `magic-market-grpc-contracts` | `magic.market.v1` Protobuf 合同 |
| `magic-market-grpc-server` | mTLS/Bearer 认证的只读 gRPC 服务 |
| `magic-tdx-rs` | 纯 Rust TDX 公共协议、历史数据和本地文件读取 |
| `magic-tdx-local-rs` | 固定 TQ-Local 客户端、监督状态机与协议合同 |
| `magic-market-monitor` | 无 I/O 的确定性异动规则与有界 replay |
| `magic-market-monitor-server` | Windows TDX 发现、轮询和监控进程 |
| `magic-market-tdx-agent` | TDX 主机到 gRPC 服务的出站 Agent |
| `magic-market-transport` | 固定 HTTPS allowlist、超时、body 上限和节流 |

其余 `magic-*-rs` crate 是各数据源的独立适配器。

## 快速开始

要求当前 stable Rust/Cargo，并安装 rustfmt 和 Clippy：

```bash
git clone https://github.com/Northofqing/magic-market-data-rs.git
cd magic-market-data-rs
cargo fetch --locked
cargo test --workspace --all-targets --locked --offline
```

构建主要服务：

```bash
cargo build -p magic-market-grpc-server --release --locked
```

运行真实数据探针示例：

```bash
cargo run -p magic-tdx-rs --example live_probe --release --locked
cargo run -p magic-tencent-rs --example live_probe --release --locked
cargo run -p magic-sina-rs --example live_probe --release --locked
cargo run -p magic-emquant-rs --example daily_bars_probe --release --locked --offline
cargo run -p magic-hithink-rs --example live_probe --release --locked
```

真实探针会访问外部数据源，不属于离线测试。部分 Provider 需要运行时凭据或厂商授权；
凭据只能放在本地环境文件或进程环境中，不能提交到 Git。

gRPC 的 TLS、Bearer Token、启动参数、客户端证书和调用示例见
[部署手册](docs/DEPLOYMENT.md)与
[gRPC 外部对接文档](docs/integrations/grpc-external-api.md)。Protobuf 文件位于
[`market.proto`](crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto)。

## 如何扩展

新增数据源或数据族按以下顺序进行：

1. 在 `magic-market-core` 复用或定义 Provider 无关的受检合同；缺失字段必须保持缺失。
2. 在独立 Provider crate 中实现固定端点、严格解析、单位转换、来源证据和有界请求。
3. 如涉及 HTTP，先完成 Gate A 设计并同步
   [`http-transports.tsv`](docs/integrations/http-transports.tsv)，不得绕过域名/路径 allowlist、
   超时、响应大小、重定向和限速策略。
4. 在 Router/Composition 中绑定质量门、切源顺序和真实 Provider，禁止下游路径依赖或
   Provider 身份伪造。
5. 在 Service 注册操作；需要外部访问时再追加 Protobuf 和 gRPC 映射。
6. 完成确定性合同测试、错误测试、有界真实 probe，并更新
   [`admissions.tsv`](docs/integrations/admissions.tsv)。

所有变更遵循 [工程规则](docs/ENGINEERING_RULES.md)和
[业务规则](docs/business_rules.md)中的 Gates A–D。未完成准入的实现只能作为显式诊断，
不能对外宣称生产可用。

## 稳定性

- 请求和分页默认原子化：任一页失败时不返回伪造的部分成功。
- 多字段快照只有在同一响应内完成身份、单位、日期和交叉字段校验后才成功；多次请求
  不会被静默拼成“原子快照”。
- 所有网络、队列、重放、响应体、分页和并发都有显式上限。
- Provider 使用固定域名/路径、超时、限速和响应校验；需要的端点由注册表机械检查。
- Provider 原始证券身份在对应适配器边界规范化；例如 CLS 的精确大写北交所
  `920403.BJ` 映射为规范北京市场身份，客户端不需要放宽或猜测代码格式。
- 数据错误使用类型化错误，不通过解析日志文本恢复业务语义。
- 缺失值不填零；当前本地时间只能写入 `observed_at`，不冒充 Provider `source_at`；
  诊断成功不提升生产准入。
- TDX 断连、终端重启、序列跳变、监控列表变化和慢消费者都会产生显式 reset 或失败。
- Core、Router 和普通 Provider 禁止 `unsafe`；唯一例外是不可发布的 Windows TDX
  只读进程发现模块，并由合规检查限定边界。
- 发布物绑定 Git SHA、工具链、目标平台和 SHA-256 清单。

项目不承诺第三方数据源永远可用。上游协议、权限或页面变化后，相关能力可能明确失败，
必须重新通过真实探针才能恢复准入。

## 性能

- TDX 提供同步、异步、直连和 Smart 客户端；连接池、分页和响应解压均有界。
- gRPC 将阻塞 Provider 放入受并发预算约束的 blocking worker，不阻塞 Tokio worker。
- TDX 本地监听使用有界队列；累计成交额慢路径与价格/成交量快路径隔离。
- 事件订阅和 replay 同时受事件数与字节数限制；慢消费者不会无限占用内存。
- Provider client 应长期复用，共享连接池和限速器；不要为每条记录创建新 client。
- Router 不做缓存或跨源聚合，避免隐藏延迟和来源污染；缓存、存储和批量调度由调用方负责。

实际吞吐取决于 Provider 限速、网络、监控标的数量和请求类型。本仓库不发布脱离这些
条件的统一 QPS/SLA 数字；部署时应使用目标数据源的 load probe 选择并发、超时和容量。

## 验证与发布

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo doc --workspace --no-deps --locked --offline
bash tools/compliance/check.sh
bash tools/docs/check_links.sh
```

完整发布门：

```bash
bash tools/release/preflight.sh
bash tools/release/package.sh
```

## 进一步文档

- [部署手册](docs/DEPLOYMENT.md)
- [gRPC 外部对接](docs/integrations/grpc-external-api.md)
- [gRPC 组合数据产品](docs/integrations/grpc-derived-products.md)
- [TDX 能力矩阵](docs/TDX_CAPABILITIES.md)
- [TDX 本地终端监听](docs/integrations/tdx-local-terminal.md)
- [同花顺扶摇 Financial API](docs/integrations/hithink-fuyao.md)
- [Provider 准入注册表](docs/integrations/admissions.tsv)
- [未准入 Provider 路径与显式替代](docs/integrations/unadmitted-provider-routes.md)
- [HTTP 传输注册表](docs/integrations/http-transports.tsv)
- [异步调用阻塞 Provider 指南](docs/integrations/async-blocking.md)

## 许可证

项目代码使用仓库声明的许可证。第三方数据、网页、SDK 和厂商组件仍受各自条款约束；
本项目的代码许可证不授予数据抓取、展示或再分发权。
