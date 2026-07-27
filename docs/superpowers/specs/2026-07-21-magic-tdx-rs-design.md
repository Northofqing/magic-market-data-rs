# magic-market-data-rs / magic-tdx-rs 设计

状态：原七个设计部分及独立仓库修订已由用户确认；本文待书面复核。Gate A 在本文被明确批准且阻塞意见归零前不关闭。

## 1. 背景

现有下游应用 `stock_analysis` 通过 `rustdx-complete = 1.0.0` 接入部分 TDX 历史行情能力，生产适配器位于其 `src/data_provider/rustdx_provider.rs`。本项目不继续扩大该应用专用适配器，而是在独立仓库中建立可复用的金融数据基础：

- umbrella 项目名：magic-market-data-rs；
- provider-neutral 核心 crate：magic-market-core；
- 完整 TDX 数据源驱动 crate：magic-tdx-rs；
- Rust 导入名遵循 Cargo 规则，分别为 magic_market_core 与 magic_tdx_rs。

magic-tdx-rs 以 jiangtaovan/tdxrs 的固定提交为行为基线，采用“审计提取并加固”，保留已验证的协议和性能关键逻辑，移除 Python 耦合，修复静默降级和不完整结果问题，并通过差分测试及同环境基准证明兼容性。

### 1.1 固定上游基线

- 仓库：https://github.com/jiangtaovan/tdxrs
- 提交：18b05ffc9d8a257b5ba5add8a2d1ab038261747d
- 上游包版本：0.6.7
- 许可证：MIT；派生代码必须保留版权和许可证声明
- 工具链基线：rolling stable，不声明固定 MSRV

固定提交是所有兼容矩阵、差分夹具和性能 A/B 的唯一基线。未来升级必须通过单独设计、重新审计和新证据完成，不能漂移跟随 main。

### 1.2 已验证的上游问题

审计发现以下行为不能直接复制：

- PyO3 为无条件依赖；在当前 macOS x86_64 环境中，上游 all-features 测试因未解析的 Python 符号链接失败。
- sync、direct 和 async 的复权路径会忽略 XDXR 获取错误，可能把未复权数据作为复权请求的成功结果。
- 复权上下文遇到传输或解析错误时可能提前停止并返回不完整上下文。
- 行情请求超过 60 个证券时会静默截断。
- 部分短字节读取返回 0，另一些路径可能 panic。
- 财务字段缺失会被填成 0.0，非法文件大小可能被转换成 0。
- 日 K 重试耗尽后仍可能成功返回空结果。

这些是采用新稳定外观和严格错误语义的直接原因。

## 2. 目标、非目标和成功标准

### 2.1 目标

首期必须：

1. 建立 provider-neutral 的强类型金融数据合同。
2. 覆盖固定上游所有纯 Rust 核心功能。
3. 将 Blocking、Async、Direct、Smart 四种客户端作为独立一等 API。
4. 对格式错误、缺失、不完整、超限、复权失败和重试耗尽提供显式类型错误。
5. 保持与固定上游同量级的速率和并发，满足第 12 节的相对 A/B 门槛。
6. 提供完整、落地、可机械验证的文档和示例。
7. 通过外部下游集成合同保持 `stock_analysis` 的业务质量、新鲜度和回退政策。
8. 支持 Rust stable，以及 Linux、macOS、Windows 的 x86_64/aarch64 目标。

### 2.2 非目标

首期不包含：

- 多 provider 聚合调度运行时；
- Python API、CLI、DataFrame 辅助层；
- 数据下载器或持久化服务；
- 下单、账户、仓位或交易接口；
- stock_analysis 的特殊业务场景；
- 对上游内部模块路径或方法签名的源码级兼容；
- 无法用固定环境复现的绝对性能承诺。

未来的金融数据聚合器位于 magic-market-core 之上，通过新的设计周期加入 provider 注册、路由、回退、合并和缓存策略，不把这些政策塞入 TDX 驱动。

### 2.3 完成定义

本独立仓库只有 Gate A、B、C、D 全部通过才可称为 crate 交付完成。缺少覆盖率、差分、性能或受控在线证据时，状态必须是“进行中/阻塞”，不得把本地单元测试通过等同于发布就绪。

`stock_analysis` 采用新 crate 是独立的下游变更，必须在其仓库内通过自己的设计、实现、合规和发布 Gate。本仓库发布就绪不等同于下游采用完成；下游验证未通过也不得阻止本仓库记录真实的库级 Gate 结果，但必须阻止宣称应用迁移完成。

## 3. 方案比较

### 3.1 方案 A：审计提取并加固（采用）

从固定上游提交提取纯 Rust 协议、解析、传输和客户端逻辑，逐模块记录来源；在稳定外观之后修复严格性问题，增加 provider-neutral 适配、差分测试和性能基准。

优点：

- 最大限度复用已经验证的协议和性能关键实现；
- 能移除 PyO3 和 Python 用户层耦合；
- 能在公共 API 稳定前修正错误语义；
- 兼容性和差异均可审计。

风险：

- 必须建立完整的来源映射和许可证保留；
- 行为加固会产生有意差异，必须逐条记录和测试；
- 全功能提取工作量较大。

### 3.2 方案 B：薄包装上游依赖（拒绝）

直接依赖 rustdx-complete 或 git revision，再提供一层 facade。

拒绝原因：无法可靠消除无条件 PyO3 链接问题；静默截断、缺失填零和复权降级仍存在；上游内部暴露面会泄漏到稳定合同。

### 3.3 方案 C：从零重写协议（拒绝）

根据协议资料和抓包重新实现全部命令。

拒绝原因：协议范围大、未知字段多，首期可靠性和性能风险高；会丢失经过实际运行验证的上游逻辑，且更难证明结果一致。

## 4. Workspace 与依赖边界

仓库根使用纯虚拟 Cargo Workspace，不定义根 package：

    magic-market-data-rs/
    ├── Cargo.toml                    # 仅 [workspace]，resolver = "2"
    ├── Cargo.lock                    # 提交，固定测试和基准依赖解析
    ├── README.md
    ├── README.en.md
    ├── crates/
    │   ├── magic-market-core/
    │   └── magic-tdx-rs/
    ├── docs/
    └── tools/

Workspace 成员只有两个可独立版本化和发布的 library crate。示例和 Criterion benchmark 放在所属 crate 的 `examples/`、`benches/` 下并参与 CI 或固定 benchmark job。根 manifest 显式列出成员和共享 lint/package 元数据，不通过 glob 意外吸收临时目录。

依赖方向固定为：

    magic-tdx-rs
        └── magic-market-core

    stock_analysis application (external downstream repository)
        ├── magic-market-core
        └── magic-tdx-rs

`magic-market-core` 不得知道 TDX 协议、服务器地址或上层应用类型。`magic-tdx-rs` 不得依赖 `stock_analysis`。下游适配器不得反向暴露进协议层，本仓库也不得通过相对路径、submodule 或构建脚本读取 `stock_analysis` 源码。

两个 crate 独立发布；本仓库不增加 umbrella facade crate。`magic-tdx-rs` 对 `magic-market-core` 使用正常 SemVer 依赖。外部生产消费者必须使用已发布的固定版本或完整 Git commit revision；本机 path dependency 只允许开发验证，不能作为发布或生产集成证据。

### 4.1 magic-market-core

核心模块：

- instrument：交易所、市场、证券标识和资产类别；
- value：Price、Quantity、Money、Ratio 等检查型值对象；
- model：Bar、Quote、Trade、Fundamental、CorporateAction 等标准模型；
- request：时间范围、周期、复权和分页请求；
- provenance：来源、抓取时间、源时间、请求标识和完整性；
- quality：标准质量问题、验证结果和 freshness 判定；
- provider：按能力拆分的 provider traits。

它只描述稳定合同和通用金融数据约束，不负责具体网络、服务器选路或项目业务回退。

### 4.2 magic-tdx-rs

内部模块：

- protocol：报文编码、解码、压缩和源记录；
- source：TDX 市场、类别、周期等源语义；
- transport：连接、超时、读写和服务器健康；
- client：四种公开客户端及其 builder；
- service：按数据能力组织的高层操作；
- adjustment：XDXR、上下文和复权；
- reader：本地 day、lc、tnf 等文件读取；
- adapter：magic-market-core Provider 实现；
- error、config：类型错误和显式配置。

协议与传输内部默认 pub(crate)。经过安全审查的低层 codec 可以进入 documented advanced API，但不把连接池 guard、内部 task、私有 packet 结构列为稳定外观。

## 5. 数据流与所有权

统一数据流：

    typed request
      -> request validation
      -> packet encoding
      -> bounded I/O
      -> strict packet validation
      -> TDX source record
      -> complete adjustment context when requested
      -> checked normalized conversion
      -> provenance-bearing DataBatch<T>
      -> consumer-specific quality/freshness gate

### 5.1 两层数据模型

TDX 源记录保留上游字段和 f64 表示，以便协议结果对照、避免不必要热路径开销。它们明确标注单位、比例、枚举含义和未知字段。

magic-market-core 标准模型使用检查型强类型：

- Price 必须有限且大于 0；
- Quantity 和 Money 必须有限，并按模型语义检查负值；
- 时间必须可解析并明确时区；
- 比例必须有清晰单位，禁止百分数与小数静默互换；
- 源缺失字段保留为 None 或返回错误，禁止填零。

转换是显式的 TryFrom/转换服务，失败包含字段和值上下文。源记录和标准记录不能依赖隐式 From 绕过检查。

### 5.2 批次、完整性和来源

DataBatch<T> 至少携带：

- provider/source 名称；
- 操作和 request id；
- server endpoint 或本地文件标识；
- fetched_at；
- source_at: Option<timestamp>；
- 请求范围、收到数量和分页统计；
- adjustment、completeness 和 quality 状态；
- 可关联的 trace id。

source_at 缺失必须保持 None。fetched_at 不能伪装成源时间。分页型高层 API 默认只有全批次完成才返回成功；允许 partial/best-effort 的 API 必须单独命名，并返回每页状态和缺失范围。

### 5.3 质量与新鲜度

通用质量层提供：

- price > 0；
- 相邻价格变化不使用固定百分比阈值拒绝；真实涨跌、上市初期波动和复权跳变必须原样保留；
- 时间间隙或重复返回错误；
- 拆股分红与序列连续性检查；
- 缺失与 not-applicable 分离。

外部下游 `stock_analysis` 的 `src/data_provider/rustdx_provider.rs` 继续执行现有 BR-092 和应用新鲜度政策。实时报价要求可证明源时间不超过 5 秒，仓位/现金 30 秒，净值同交易日，日线/历史不超过 1 个交易日。TDX 无可信源时间时返回 `None`，由应用判为不可证明；不得使用本地抓取时间冒充。

## 6. 公共 API 与稳定性

### 6.1 客户端

公开四种独立类型：

- BlockingClient：固定大小同步连接池；
- AsyncClient：Tokio 每连接任务和有界通道池；
- DirectClient：每请求独立连接；
- SmartClient：服务器探测、健康、黑名单和重试政策。

它们通过 Builder/Config 构建，显式配置服务器、超时、连接数、限速、重试预算和缓存。构造不读取不可见全局配置。

不使用 magic integer 表示 market、category、period 或 adjustment；公开接口使用非穷尽 enum、新类型或检查型 request。

### 6.2 能力接口

magic-market-core 的初期 provider traits：

- InstrumentProvider；
- HistoricalBars；
- RealtimeQuotes；
- MinuteData；
- Trades；
- Fundamentals；
- CorporateActions；
- FundData；
- BlockData；
- ProfileData。

trait 按能力拆分，避免一个 provider 因不支持某个领域而伪造结果。magic-tdx-rs 的 TdxProvider 组合已配置客户端并实现实际支持的 traits。

### 6.3 方法语义

- quotes 对超过 60 个证券的输入返回 InvalidRequest；
- quotes_chunked 显式分块并返回可核对的映射和批次元数据；
- 调整请求只有取得完整且有效的 XDXR/context 才成功；
- Adjustment::None 不额外获取 XDXR；
- 无数据、空响应、字段不适用和字段缺失是不同状态；
- 返回列表的排序、重复和分页语义在方法文档中固定；
- 取消、超时和任务 join 错误不可丢失。

### 6.4 SemVer 合同

稳定面位于 crate 顶层 facade 和 prelude：

- 公开错误和 enum 使用 non_exhaustive；
- 配置使用 builder，新增默认字段不破坏调用方；
- 记录字段优先只读访问器；需要公开字段时明确 SemVer 影响；
- 公共 API 通过 cargo-semver-checks 检查；
- 开发版本从 0.1.0 开始，合同、兼容矩阵、性能和在线验证稳定后才评估 1.0。

## 7. 完整功能范围

magic-tdx-rs v1 必须覆盖固定上游全部纯 Rust 能力：

- 股票和指数实时行情；
- 日线、周期 K 线、分时、分钟历史、逐笔/成交；
- 证券数量、证券列表和市场元数据；
- 财务信息、财务文件列表和指标；
- XDXR、除权除息和前/后复权；
- ETF/基金行情与相关查询；
- 板块文件和板块查询；
- F10/profile 内容；
- 本地 day、lc、tnf 等上游支持的读取器；
- Blocking、Async、Direct、Smart 四种客户端。

COMPATIBILITY.md 为每个上游 Rust-callable 操作记录 Adopt、Replaced、Intentional Difference 或 Deferred。首期纯 Rust核心项目不得出现未解释的 Deferred。

## 8. 协议完整性与错误

### 8.1 Bounds-checked codec

所有读取通过 ByteCursor 或等价的有界 cursor：

- read 方法全部返回 Result；
- 错误包含字段和 byte offset；
- 检查 header、长度、记录数和压缩尺寸；
- 限制解压后最大尺寸，阻止压缩炸弹；
- 固定宽度记录要求 exact length；
- 可变记录必须完整消费声明字段；
- trailing bytes 只能在协议文档明确允许时存在；
- 任意输入不得 panic、越界或无限分配。

未知字段保留原始值并在 PROTOCOL.md 标记证据状态，不猜测语义。

### 8.2 错误分类

公开错误族：

- Configuration；
- InvalidRequest；
- Transport；
- Protocol；
- Decode；
- Decompression；
- RateLimited；
- PoolExhausted；
- NoData；
- EmptyResponse；
- IncompletePage；
- Adjustment；
- RetryExhausted；
- Unsupported。

错误上下文按可用性携带 operation、instrument、server、attempt、field、offset、request id 和底层 source。每种错误有明确 retryability；调用方不通过字符串判断是否重试。

### 8.3 失败政策

- malformed/truncated packet：错误；
- 声明记录与实际不一致：错误；
- 字段缺失：None 或错误，取决于字段合同，禁止填零；
- 超限请求：错误；
- 分页中任一页失败：默认整个请求错误；
- 复权上下文不完整：错误；
- 重试耗尽仍为空：EmptyResponse 或 RetryExhausted；
- 服务器返回明确“无数据”：NoData；
- 显式 best-effort：返回结构化不完整信息，不复用严格方法名。

公共 API 不包含 unwrap/expect 驱动的可达 panic，不吞掉 spawned task 错误，不把错误降级成空 Vec。

## 9. 并发、背压与限速

### 9.1 BlockingClient

- 默认连接池大小 5，与固定上游默认行为对齐；
- 借用连接具有 pool timeout；
- 网络 I/O 期间不持有全局管理锁；
- 失败或协议失步的连接被丢弃，不放回池；
- 请求队列/等待受 overall timeout 约束。

### 9.2 AsyncClient

- 默认 4 个连接 task；
- 每个连接拥有单独 bounded channel；
- 调度采用明确并可测试的 round-robin；
- channel 满时产生背压或配置化超时，不建立无界队列；
- 取消立即传播到等待者；
- task panic、退出和 heartbeat 失败均显式返回。

### 9.3 DirectClient

- 每次请求建立独立连接；
- 无共享池和共享串行瓶颈；
- 并发上限由调用方或显式 semaphore 配置；
- connect/write/read/overall timeout 均生效；
- 适合作为高并发对照，但不承诺规避服务器限制。

### 9.4 SmartClient

- server 健康、失败计数、冷却和黑名单状态可观察；
- 整个操作共享总重试预算，不能每个内部层重新计数；
- 服务器排序、过滤和切换语义注册业务规则；
- 所有服务器失败时返回聚合错误，不返回最后一次空结果；
- 包装哪些 capability 必须由 trait/方法明确，不通过 inner() 暗示完整统一能力。

### 9.5 自适应限速

兼容默认策略支持 15/30/60 req/s 阶段，上限不超过 200 req/s。作用域必须显式选择：

- PerClient；
- PerConnection。

默认值、交易阶段和选择规则登记到 docs/business_rules.md。时区固定 Asia/Shanghai，时钟可注入。无法确定交易阶段时采用保守速率。基准分别报告：

1. 与固定上游相同限速配置下的端到端结果；
2. 双方显式关闭 limiter 后的实现开销。

不得只展示关闭限速的结果来宣称生产吞吐等价。

## 10. 缓存、时间和可观测性

- 缓存默认关闭或由显式 CachePolicy 开启；
- cache hit/miss、缓存年龄和来源进入批次元数据或 trace；
- 过期缓存不得作为成功的新鲜数据；
- 服务器健康和限速状态有只读快照；
- tracing 字段包含 operation、request id、server、attempt、elapsed、record count；
- 不记录敏感账户信息；本项目首期没有账户和交易能力；
- 审计所需 provenance 由驱动提供；本仓库 Gate D 验证字段完整性、可追踪性和文档合同，不伪造持久化能力。
- 每个生产消费者负责把 provenance 接入其审计设施。`stock_analysis` 的采用 Gate 必须证明审计防篡改且保留不少于 5 年；缺少证据时阻塞下游发布，但不把消费者基础设施伪装成本仓库能力。

## 11. 测试与兼容性验证

### 11.1 确定性测试层

- unit：编码、解析、转换、调整、限速、连接状态和错误分类；
- golden fixture：固定二进制输入、期望源记录、SHA-256 和来源说明；
- differential：同一输入由固定上游和 magic-tdx-rs 解析，对比完整字段；
- property/fuzz：随机和截断字节永不 panic、越界或无界分配；
- protocol replay：本地 loopback server 覆盖分页、截断、超时、断连、空响应和重试；
- reader fixture：全部本地格式、边界长度和损坏文件；
- integration：四客户端和所有能力的组合测试；
- downstream contract：提供可由外部消费者复用的契约夹具；`stock_analysis` 自己验证 BR-092、新鲜度、分页及回退不回归，不把其源码纳入本 workspace 测试。

测试数据使用 TEST_CODE 前缀或明确的二进制 fixture。生产路径不得引用测试 server、fixture 或 mock feature。

### 11.2 上游差分基线

固定上游在隔离目录编译。为纯 Rust 差分 harness 允许应用一个最小“移除 PyO3 注册/构建耦合”补丁，但必须：

- 固定上游 commit；
- 保存补丁文件和 SHA-256；
- 证明补丁不改变协议、解析、客户端或数值逻辑；
- 在 UPSTREAM.md 记录构建命令和差异；
- 不把修复后的上游误称为原始发布包。

pytdx、mootdx 或抓包只用于解释协议歧义；它们不能取代固定上游作为主要兼容基线。

### 11.3 在线验证

在线验证是显式、只读、人工运行的 diagnostic binary，不属于默认 cargo test：

- 使用真实公共行情服务器；
- 不下单、不访问账户；
- 输出脱敏 server、时间、结果数量、错误分类和统计；
- 原始数据按仓库权限与审计要求保存；
- 非交易时段或网络不可用必须报告阻塞，不能伪造成功。

## 12. 性能验收

### 12.1 原则

“与 tdxrs 一致”定义为固定上游与本实现在同一机器、同一 Rust profile、同一服务器/fixture、同一请求和同一限速配置下交替 A/B。上游 README 的历史绝对数字只作背景，不是验收阈值。

### 12.2 测试矩阵

- codec/reader：固定 fixture 的解析吞吐和分配；
- loopback client：Blocking、Async、Direct、Smart，在 1、5、60 并发；
- 批量行情：1、5、60 个证券和显式 chunked 路径；
- bars/minute/trades/finance/XDXR/readers 的代表性请求；
- live：固定可用 server 上的交替请求。

所有比较使用 release profile，固定依赖 lock，记录 OS、CPU、架构、Rust 版本、commit、服务器、连接数、timeout、limiter、warm-up、样本数和原始 JSON。

### 12.3 门槛

- 确定性 codec/reader 与 loopback client 吞吐回归不得超过 5%；
- 1/5/60 并发分别按客户端策略与固定上游对比，不要求 pooled 模式伪装成 Direct；
- 受控 live median 和 p95 延迟回归不得超过 10%；
- live 成功率不得低于固定上游；
- 任何比较若样本不足、服务器变化或限速配置不同，则证据无效；
- 内存、队列深度和错误率一并报告，不能以无界缓存换取吞吐。

若加固检查使某操作超过门槛，必须先定位开销并优化；不能删除完整性校验来通过性能门。

## 13. 文档体系

文档和代码同时交付，不允许发布前临时补写。

仓库级专题文档统一放在 `docs/`，crate 自身 README、rustdoc 和 examples 与源码同目录。根 README 是独立项目入口，不复制或删除外部 `stock_analysis` 的应用文档。

根和 crate 文档：

- README.md：umbrella 定位、架构、快速开始、crate map、安全边界、兼容性和性能摘要；
- README.en.md：英文入口和 API synopsis；
- crates/magic-market-core/README.md；
- crates/magic-tdx-rs/README.md；
- 各 crate `examples/` 中可编译的能力示例。

专题文档：

- ARCHITECTURE.md；
- API_GUIDE.md；
- DATA_MODEL.md；
- PROTOCOL.md；
- ERROR_HANDLING.md；
- CLIENTS_AND_CONCURRENCY.md；
- RATE_LIMITING.md；
- COMPATIBILITY.md；
- MIGRATION_FROM_TDXRS.md；
- UPSTREAM.md；
- PERFORMANCE.md；
- TESTING.md；
- OPERATIONS.md；
- SECURITY.md；
- CONTRIBUTING.md；
- CHANGELOG.md；
- SUPPORT.md。

文档要求：

- 全部公开 item 有 rustdoc；
- crate 启用 deny(missing_docs) 和 broken intra-doc link 检查；
- 示例参与编译或 doctest；
- 协议结论注明上游代码、fixture、差分、抓包或外部文档证据；
- 未知字段明确标记未知；
- 性能结论关联机器可读原始结果；
- 兼容矩阵尽可能从测试 inventory 生成；
- CI 运行 cargo doc、doctest、examples 和链接检查；
- 首版中文技术文档完整，另提供英文 README 和公开 API 摘要；不维护两套易漂移的全文副本。

## 14. 外部下游 `stock_analysis` 集成

`stock_analysis` 的 `src/data_provider/rustdx_provider.rs` 继续作为其项目政策边界：

- 保留 BR-092；
- 保留整页/整批严格校验；
- 保留 5 秒实时和 1 个交易日日线新鲜度要求；
- 保留项目现有 source name 和上层 fallback 顺序；
- TDX 不提供可信 realtime source timestamp 时，不改变当前使用可信来源的决定；
- 不把项目特有过滤、选择、回退或人工确认逻辑下沉到通用 crate。

下游集成使用独立 PR 和独立提交，不与本仓库协议/模型提交混合。依赖必须固定为已发布版本或完整 Git revision；本机 path dependency 只可用于预提交开发，不能进入生产 manifest 或作为 Gate 证据。

应用集成前，新旧驱动可以通过独立构建目标进行 A/B，但生产调用链不得在新驱动失败时静默调用旧驱动。切换后的错误必须进入现有显式 fallback 政策。下游必须独立运行自己的 format、Clippy、测试、compliance、freshness、覆盖率、真实数据和审计验证。

### 14.1 旧模块关系

| 模块 | 决策 | 原因 |
| --- | --- | --- |
| `stock_analysis/src/data_provider/rustdx_provider.rs` | adopt and adapt downstream | 保留应用质量、新鲜度、分页和回退政策 |
| `stock_analysis` 的 `rustdx-complete 1.0.0` | remove downstream after evidence | PyO3 耦合和静默失败语义不满足目标 |
| 固定上游纯 Rust 协议/解析逻辑 | audited extraction | 保留兼容和性能基础 |
| 上游 Python/CLI/DataFrame/downloader | reject | 不属于首期纯 Rust 驱动 |
| `stock_analysis` 特殊场景 | retain in downstream application | 防止通用驱动被业务政策污染 |

## 15. 业务规则与数据红线

实现第一阶段必须在本仓库落地独立的 `AGENTS.md`、工程 Gate 文档和 `tools/compliance/check.sh`。规则编号保留 2.1–2.10 以便追溯原始安全约束，但只定义本 library workspace 适用的检查；不得通过读取外部仓库文件来获得政策。随后创建并检查本仓库 `docs/business_rules.md` 的当前最大编号，再登记：

- 单次行情 60 个证券限制和 quotes_chunked；
- 分块后的顺序、重复代码和结果映射；
- 分页完整性和严格原子结果；
- Smart 服务器筛选、排序、冷却和总重试预算；
- 连接池选择、round-robin 和 bounded queue；
- 15/30/60 自适应限速、200 上限和作用域；
- 缓存启用、年龄及命中可见性。

规则映射：

- 2.1：生产路径无 mock/fixture；源失败显式返回。
- 2.2：缺失保持 None 或错误，禁止填零。
- 2.3：强类型数值、OHLC/量额、时间连续性和 XDXR 一致性；不以固定涨跌幅替代来源一致性检查。
- 2.4：本仓库保证来源时间和抓取时间分离；`stock_analysis` 在自己的 Gate 中保持 5 秒/1 交易日门。
- 2.5：测试数据 TEST_CODE；本 crate 不含下单能力。
- 2.6：不适用；没有订单 API。
- 2.7：`DataBatch` 提供 provenance/trace；生产消费者负责持久化审计并在自己的 Gate 中举证。
- 2.8：verify/save/notify/push/sync/update_result/reconcile 等命名若出现，必须真实操作目标；禁止日志占位。
- 2.9：本设计不修改 config/*.toml 阈值；未来修改必须提供 spec/config 双向证据。
- 2.10：上述限速、分块、排序、筛选、去重和池策略必须先登记 BR。

## 16. 实施顺序与 Gate

### 16.1 阶段

1. 落地本仓库 `AGENTS.md`、工程 Gate、业务规则入口和 compliance 骨架；初始化纯虚拟 Workspace，提交根 lockfile，仅加入两个 library crate 骨架，不定义根 package。
2. 完成 magic-market-core 值对象、模型、provenance、quality 和 capability traits。
3. 按来源清单提取 codec、源类型、reader 和 adjustment，先建 golden/differential 证据。
4. 完成 transport 及四客户端，覆盖取消、超时、断连、背压和重试。
5. 完成全部数据能力、严格错误和标准化 adapter。
6. 完成文档、示例、兼容矩阵和 benchmark harness。
7. 完成本仓库 Gate B/C/D，发布固定版本或记录可审计的完整 Git revision。
8. 在 `stock_analysis` 仓库另开设计和集成 PR，使用固定依赖完成 A/B 与应用 Gate。
9. 只有下游 Gate 全部通过后，才在 `stock_analysis` 移除 `rustdx-complete`。

### 16.2 Gate A

- 本文和后续实施计划已批准；
- 数据流、失败模式、旧模块关系和回滚明确；
- 阻塞意见为 0；
- 业务规则登记位置明确。

### 16.3 Gate B

- 功能矩阵无未解释空缺；
- 失败路径可执行；
- 没有生产 mock、空实现或静默降级；
- workspace fmt、strict Clippy 和 tests 通过。

### 16.4 Gate C

以下全部通过：

    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-targets --all-features
    cargo test --workspace --doc --all-features
    cargo doc --workspace --all-features --no-deps
    bash tools/compliance/check.sh

若 workspace all-features 与平台 feature 互斥，必须在实施计划中显式拆矩阵，不能跳过检查。

本仓库的 `tools/compliance/check.sh` 只检查适用于独立 library workspace 的规则，包括生产 fixture/mock 隔离、假实现、设计矛盾、业务规则、许可证和来源映射。它不得读取外部数据库或依赖 `stock_analysis` 的 freshness/backfill 脚本。下游 freshness 失败仍在 `stock_analysis` 中严格阻塞其合并。

### 16.5 Gate D

- overall coverage >= 80%；
- 核心协议、解析和数据链路 >= 95%；
- cargo-semver-checks、依赖许可证和安全审计通过；
- 跨平台/stable CI 通过；
- 差分和性能门槛通过；
- 受控在线只读验证通过；
- 文档、示例和链接通过；
- 审计人签字；
- PR 证据字段完整。

覆盖率至少执行：

    cargo llvm-cov --workspace --all-features --json --output-path target/coverage/coverage.json -- --test-threads=1
    python3 tools/coverage/check_thresholds.py target/coverage/coverage.json

差分、基准、文档链接、SemVer、许可证和依赖安全命令必须由实施计划绑定到仓库内脚本或固定 CI job；只写“人工检查”不能作为 Gate D 证据。

### 16.6 下游采用 Gate

`stock_analysis` 集成不属于本 workspace 的 Gate B/C/D。它必须在外部仓库独立满足：

- 固定 crate 版本或完整 Git revision，禁止生产 path dependency；
- BR-092、整页/整批、新鲜度和 fallback 回归测试；
- format、strict Clippy、全部测试和该仓库 compliance；
- freshness/backfill、真实数据、生产调用链和审计保留证据；
- 单独可回滚的适配器/依赖提交。

任一项缺失时，只能报告“库已就绪、下游采用进行中/阻塞”，不能报告应用迁移完成。

## 17. 回滚

每个阶段使用独立小提交。本仓库的协议、模型和发布提交与外部应用切换严格分离；下游切换必须在 `stock_analysis` 使用单独提交。

回滚顺序：

1. 下游验证失败：在 `stock_analysis` 执行 `git revert <integration-commit>`，恢复已固定的旧依赖调用路径。
2. crate 行为问题：下游保持旧路径；本仓库 `git revert <library-commit>` 或发布修复版本，再重新差分。
3. 架构或数据流问题：返回 Gate A，修订本文和实施计划。
4. 数据红线问题：停止发布，Gate B 修复后重新检查 Gate A 失败模式。

本设计不修改数据库 schema、不迁移账户/交易数据、不改变订单路径。禁止通过关闭质量门、填充假数据或静默回退完成“回滚”。

本仓库标准回滚命令：

    git revert <library-commit>
    cargo test --workspace --all-targets --all-features

下游标准回滚命令（在 `stock_analysis` checkout 内执行）：

    git revert <integration-commit>
    cargo build --release

实际提交 SHA 在 PR Rollback 字段填写。

## 18. PR 证据

本仓库 PR 描述必须包含：

- Refs：本文具体章节及实施计划；
- Data-Redlines：2.1、2.2、2.3、2.4、2.5、2.7、2.8、2.10；
- OldModules：固定上游的 Adopt/Reject/Replace 结果；外部应用项标记为 downstream，不伪装成本 PR 已执行；
- Threshold-Proof：第 9.5 和第 12 节的配置、基准和证据；
- Business-Rules：实际分配的 BR 编号；
- Validation：Gate B/C/D 命令和结果；
- Rollback：精确提交和构建步骤。

`stock_analysis` 的集成 PR 另行引用第 14 节，并按其仓库模板提供 Data-Redlines、OldModules、freshness、生产证据和回滚字段。

## 19. 风险与缓解

| 风险 | 缓解 |
| --- | --- |
| 协议字段未知或解释错误 | 保留原始值，标记未知，通过多来源证据再稳定语义 |
| 加固导致性能下降 | 先定位分配/锁/I/O 开销，保留校验，按 5%/10% 门槛优化 |
| live server 波动使基准无效 | 同 server 交替 A/B，记录配置和原始结果，证据不足则阻塞 |
| PyO3 阻碍上游差分 | 固定最小去耦补丁及 SHA，证明不改核心逻辑 |
| 公共 API 过早冻结 | 0.x 阶段、窄 facade、internal pub(crate)、SemVer 检查 |
| 文档漂移 | 编译示例、生成兼容矩阵、链接/rustdoc CI |
| 下游应用特殊规则污染通用 crate | 保持独立仓库、固定依赖和单向适配器边界 |
| 无界并发或重试放大 | bounded queue、overall timeout、共享 retry budget |
| 缺失字段被误认为零 | Option/typed error 和差分失败测试 |

## 20. 最终验收清单

- [ ] 两个 crate 的公共 API、功能矩阵和文档完整。
- [ ] 固定上游纯 Rust 能力全部 Adopt/Replaced/Intentional Difference。
- [ ] 四种客户端的正确性、取消、背压和并发测试通过。
- [ ] 任意或截断协议输入不 panic、不越界、不无界分配。
- [ ] 缺失、不完整、超限、复权失败和空重试均显式。
- [ ] source_at 不被 fetched_at 替代。
- [ ] 本仓库生产路径没有 mock、fixture 或日志占位。
- [ ] 确定性性能回归 <= 5%。
- [ ] live median/p95 回归 <= 10%，成功率不降低。
- [ ] Rust stable 和目标 OS/架构矩阵通过。
- [ ] overall coverage >= 80%，核心链路 >= 95%。
- [ ] 文档、rustdoc、doctest、examples、链接和 SemVer 检查通过。
- [ ] 本仓库适用的 compliance、许可证、来源映射和依赖安全审计通过。
- [ ] provenance 合同及字段完整性已验证；未把下游持久化能力记为本仓库能力。
- [ ] 受控在线验证和审计人签字完成。
- [ ] PR 所有证据字段完整，回滚命令可执行。

以上库级清单任一项缺失时，本仓库不得报告 crate 交付完成或发布就绪。

`stock_analysis` 采用完成还必须满足：

- [ ] 使用固定版本或完整 Git revision，无生产 path dependency。
- [ ] BR-092、新鲜度、分页和 fallback 顺序未回归。
- [ ] 没有静默旧驱动回退，freshness/compliance/真实数据验证通过。
- [ ] provenance 已接入可证明防篡改且保留不少于 5 年的审计设施。
- [ ] 下游集成提交可独立回滚。

下游清单任一项缺失时，不得报告 `stock_analysis` 采用完成；这不追溯否定已经取得的本仓库库级证据。
