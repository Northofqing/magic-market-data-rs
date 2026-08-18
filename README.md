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
- 为每条记录保留 Provider、批次、源时间（源能够证明时）、本地观测时间、质量状态和
  完整性证据。

精确到数据族和 Provider 的当前状态以
[准入注册表](docs/integrations/admissions.tsv) 为准；gRPC 请求与响应见
[外部对接文档](docs/integrations/grpc-external-api.md)。

## 明确边界

本项目只处理市场数据，不提供：

- 策略回测、参数优化、撮合、滑点、手续费、资金曲线或绩效归因；
- 模拟交易、实盘下单、撤单、账户、资金、持仓或成交回报；
- 数据库、数据湖、长期历史仓库或跨请求缓存；
- 对公共网页、厂商服务可用性和数据再分发权的 SLA 承诺；
- 未经证明的字段推导、跨源静默拼接或未授权接口访问。

当前没有 Level-2 集合竞价权限、CFETS DR007 机器接口授权和 IMF beta/SDMX
账户合同。这三类合同与字段继续保留，但生产调用明确返回 `UNADMITTED`/不可用，字段保持
空值；已有的 CFETS Shibor、LPR 和官方汇率能力不受 DR007 授权缺口影响。

TDX 历史 K 线、分时、逐笔、财务和公司行动可以作为外部回测系统的数据输入，但
“能够读取历史数据”不等于本仓库已经实现回测引擎。

TDX 本地终端集成只访问固定的 `http://127.0.0.1:17709/` 只读方法，不开放该端口，
不加载厂商 DLL，不依赖 Python，也不调用账户或交易接口。当前生产准入覆盖
`Now`、`Volume`、`Amount`、昨收、开高低和带完整 Core 证据的三类异动触发；源时间和
`source_record_count` 仍不可用，预热、冷却和 reset 不冒充异动触发。

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
- 所有网络、队列、重放、响应体、分页和并发都有显式上限。
- Provider 使用固定域名/路径、超时、限速和响应校验；需要的端点由注册表机械检查。
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
- [Provider 准入注册表](docs/integrations/admissions.tsv)
- [HTTP 传输注册表](docs/integrations/http-transports.tsv)
- [异步调用阻塞 Provider 指南](docs/integrations/async-blocking.md)

## 许可证

项目代码使用仓库声明的许可证。第三方数据、网页、SDK 和厂商组件仍受各自条款约束；
本项目的代码许可证不授予数据抓取、展示或再分发权。
