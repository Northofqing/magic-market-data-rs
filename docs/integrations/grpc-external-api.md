# Magic Market gRPC 对接文档

## 1. 当前状态

| 项目 | 状态 |
| --- | --- |
| Protobuf v1 合同 | 已建立，可生成客户端 |
| 60 个只读数据族 RPC | 已进入 v1 Proto；新增 `InstrumentNews` 与 5 个组合数据产品接口 |
| 能力与健康接口 | 已进入 v1 Proto |
| TDX 动态监控列表、异动订阅、重放、Agent 流 | 已进入 v1 Proto |
| gRPC Server | 已实现并在当前 Windows 工作站运行受限联调实例 |
| Unary Provider composition | 60 个操作精确登记；59 个操作至少有一个正式 handler；`EconomicCalendar` 因金十免费日历/API 已退役而仅保留显式诊断路径；Provider 备选与诊断状态由 `GetCapabilities` 精确返回 |
| TDX 数据/异动正式准入 | 价格、累计成交量、累计成交额、昨收、OHLC 与三类带 Core 证据的 trigger/rearm 事件为生产数据；状态消息仍为 `UNADMITTED` |

另一个项目现在可以根据 Proto 生成客户端并连接当前受限联调实例。实例地址、证书和
Token 仍属于部署材料而不是稳定公共地址；迁移主机、IP 或证书后必须重新交付连接包。

## 2. 合同源文件

唯一合同源：

```text
crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto
```

Protobuf package：`magic.market.v1`，当前协议版本：`1`。

调用方不得复制并自行修改 Proto。升级时以仓库内文件和 descriptor set 为准。

## 3. 网络地址

本机诊断：

```text
http://127.0.0.1:<operator-port>
```

远程部署：

```text
https://<server-name>:<operator-port>
```

- 服务不约定固定端口，端口由部署方显式提供。
- 默认只允许 loopback。
- 非 loopback 必须启用双向 TLS（mTLS）和 Bearer 认证。
- 客户端不得直接访问 TDX 的 `127.0.0.1:17709`。

当前工作站联调实例为 `https://10.211.55.3:50051`，TLS server name 是
`magic-market.local`；仅允许配置的局域网网段并强制 mTLS + Bearer。客户端材料在
服务端本机 `target/runtime/client-bundle/`，不得提交到 Git 或通过公开渠道传输。

## 4. 认证

业务客户端通过 gRPC metadata 发送：

```text
authorization: Bearer <token>
```

远程环境强制使用 mTLS；Bearer Token 是额外的应用身份，不得放进 Protobuf 请求体、
URL、日志或错误信息。服务端发布时会同时交付服务 DNS、TLS CA/证书链、客户端身份
以及服务端配置的消息、并发和流上限。

## 5. 通用请求合同

所有查询 RPC 使用 `QueryRequest`：

```proto
message QueryRequest {
  RequestContext context = 1;
  string preferred_provider = 2;
  CanonicalPayload payload = 3;
  bool allow_unadmitted = 4;
}
```

### RequestContext

```text
protocol_version = 1
request_id       = 调用方生成的非空唯一请求 ID
```

同一业务重试应保留原 `request_id`，并由调用方另外记录 retry attempt。

### preferred_provider

- 空字符串：由服务端正式 Composition/Router 选择来源。
- 非空：必须精确匹配服务端已登记 Provider。
- 不能填写 URL、IP、代理、任意 Provider 名称或动态插件名。

建议普通调用保持为空。

### allow_unadmitted

- 默认 `false`：未准入能力继续在 Provider I/O 前返回 `UNIMPLEMENTED`；
- `true`：只允许执行服务端预先登记的诊断 handler，不能注入 URL/方法/Provider；
- 诊断响应必定是 `admission=UNADMITTED`、`complete=false`，并返回
  `diagnostic_blocker`；
- 该开关只用于开发联调和证据采集，不能用于生产告警或交易决策。

服务端把 `--provider-timeout-ms` 与 `--blocking-deadline-ms` 分开配置。前者约束每次
Provider 网络调用，后者约束包含分页、解析和规范化在内的完整阻塞任务；前者不得大于
后者。这样全市场诊断可以拥有更长的有界总预算，而不会放宽任何 Provider 的单请求
HTTP 超时。

### CanonicalPayload

```text
schema         = 方法登记的请求 schema 名称
schema_version = 正整数；大多数合同为 1，新闻合同按下文使用 2
content_type   = application/json; charset=utf-8
data           = UTF-8 JSON 字节
```

第一版 gRPC 使用 Protobuf 作为传输和服务合同，现有 Rust Serde JSON 作为每个数据族的
规范化业务 payload。每个方法的 schema 名称和 JSON 字段在服务端 Provider 接入时
单独冻结；调用方遇到未知 schema/version 必须停止解析，不能忽略或猜字段。

### 新闻合同 v2

`GlobalNews` 的请求 schema 是 `magic.market.global_news.request`、版本必须为 `2`，业务
JSON 为 `{"limit":N}`。返回的每个 `magic.market.news_item` 记录同样是版本 `2`，并且
必须携带该条记录自己的完整 `evidence`。`QueryResponse.source_at` 只表示批次中最新记录
的来源时间，绝不能用它创建、补齐或覆盖逐条 evidence。

以下是两条发布时间不同的完整响应业务示例；Provider 原始 `source_at` 字符串保持不变，
而 `published_at` 可以规范化为 RFC3339，但两者必须表示同一时间点：

```json
{
  "source_at": "2026-08-19 16:15:37",
  "records": [
    {
      "schema": "magic.market.news_item",
      "schema_version": 2,
      "data": {
        "item_id": "REDACTED_NEWS_001",
        "title": "redacted title 1",
        "summary": null,
        "content": null,
        "publisher": "Jin10",
        "url": "https://example.com/redacted/1",
        "published_at": "2026-08-19T16:15:37+08:00",
        "instruments": [],
        "topics": [],
        "language": "zh-CN",
        "evidence": {
          "provider": "Jin10",
          "source_at": "2026-08-19 16:15:37",
          "observed_at": "1787127606.533354000",
          "batch_id": "REDACTED_GLOBAL_NEWS_BATCH"
        }
      }
    },
    {
      "schema": "magic.market.news_item",
      "schema_version": 2,
      "data": {
        "item_id": "REDACTED_NEWS_002",
        "title": "redacted title 2",
        "summary": null,
        "content": null,
        "publisher": "Jin10",
        "url": "https://example.com/redacted/2",
        "published_at": "2026-08-19T16:14:00+08:00",
        "instruments": [],
        "topics": [],
        "language": "zh-CN",
        "evidence": {
          "provider": "Jin10",
          "source_at": "2026-08-19 16:14:00",
          "observed_at": "1787127606.533354000",
          "batch_id": "REDACTED_GLOBAL_NEWS_BATCH"
        }
      }
    }
  ]
}
```

服务端在序列化任何记录前原子校验完整批次：逐条 Provider 和 batch ID 必须与请求/响应
一致；`source_at` 必须可按该 Provider 的真实格式解析、不得晚于该记录 `observed_at`，且
必须与 `published_at` 表示同一时间点；记录必须从新到旧，批次 `source_at` 必须等于首条
记录的原始来源字符串。缺失、混批、冲突、用批次最新时间冒充较早记录时间时，整批返回
`FAILED_PRECONDITION`、`reason_code=invalid_evidence`、`retryable=false`，不返回部分记录。

已验证并保留的 GlobalNews 原始时间格式包括：Eastmoney `YYYY-MM-DD HH:MM`；Jin10、
XinhuaFinance `YYYY-MM-DD HH:MM:SS`；Yicai 的 Provider 本地时间；CLS epoch 秒；
ThePaper `unix-ms:<毫秒>`；`observed_at` 保留秒/纳秒格式。

`InstrumentNews` 请求 schema 是 `magic.market.instrument_news.request`、版本必须为 `2`：

```json
{
  "instrument": {"exchange":"Shanghai","code":"600000","asset_class":"Equity"},
  "start": "2026-08-19",
  "end": "2026-08-19",
  "limit": 20,
  "captured_through": "2026-08-19T16:15:37+08:00"
}
```

`captured_through` 是调用方捕获的精确 RFC3339 截止时刻，不是天数。start/end 必须同时
提供或同时省略；提供时 end 必须等于截止时刻在 Asia/Shanghai 的日期。服务端会剔除
任何发布时间晚于该时刻的记录，并从保留后的最新记录重建批次 `source_at`。服务端在
过滤前验证完整上游批次，不能用 cutoff 隐藏错误 evidence。若合法截止后无记录，返回
`ADMITTED`、`complete=true`、`records=[]`，保留真实上游 `batch_id` 与 `observed_at`，并将
批次 `source_at` 留空；该 verified-empty 不是 `invalid_evidence`。

## 6. 通用响应合同

`QueryResponse` 包含：

```text
request_id
operation
admission
selected_provider
batch_id
complete
observed_at
source_at
records[]
diagnostic_blocker
```

调用规则：

- `admission=ADMITTED` 才能作为生产数据使用。
- `complete=false` 不能被当作成功完整批次。
- `source_at` 为空表示来源没有提供可信源时间；不能用 `observed_at` 代替。
- `records[]` 中每项都有独立 schema/version/content-type。
- `diagnostic_blocker` 非空表示本次是显式诊断读取；即使 records 非空，也不能视为准入。
- `batch_id`、Provider、单位和来源证据必须原样保存。

## 7. 服务与方法

### SystemService

```text
GetCapabilities
GetHealth
```

启动后应先调用 `GetCapabilities`。RPC 存在不等于对应能力已经准入；每个能力同时返回
repository admission、runtime availability、diagnostic availability、精确范围和 blocker。

`GetHealth.observability` 是 append-only 的运行时观测快照，包含进程启动 Unix 毫秒、
单调 uptime、query started/succeeded/failed/cancelled/in-flight/rejected/timed-out、累计与最大
耗时微秒，以及 unary/blocking 并发上限和当前可用 permit。计数是进程生命周期聚合值，
没有 Provider、证券、request_id 或 payload 标签；平均耗时由调用方使用
`query_duration_micros_total / (query_succeeded + query_failed)` 计算。旧客户端忽略新增字段，
不得把这些运行时值当作 Provider 时间、数据 evidence 或准入依据。

`GetHealth.build_identity` 用于核对实际运行产物：`service_version`、构建时可用的
`source_revision`、protobuf descriptor 的 `contract_sha256`、当前进程二进制的
`binary_sha256`，以及仅在无法读取二进制身份时出现的 `identity_error`。monitor、探针和
客户端 bundle 必须比较这些字段，不能仅凭进程名或本地源码目录推断服务版本。二进制摘要
在服务进程内首次计算并缓存，不进入行情查询热路径。

本服务不持久化查询审计。客户端若同时保存请求审计与服务响应审计，必须以稳定
`request_id`/batch identity 去重，不能把同一 `HistoricalBars` 批次作为两次独立采集写入。
数据库增长、保留周期和重复落库属于客户端数据基础设施；服务端负责返回一次完整响应及其
证据。持仓 FIFO、T+1 可卖账本和 `invalid_position_ledger` 也属于账户/策略系统，不由本
市场数据服务推导或修补。

### MarketDataService

```text
HistoricalBars             MinuteData
RealtimeQuotes             MoneyFlows
OrderBooks                 Auctions
Trades                     SecurityMetadata
GlobalIndices              ForeignExchange
EconomicCalendar           FuturesDelivery
ReferenceRates             OfficialFxFixings
EconomicSeries             CompanyFilings
GlobalNews                 Announcements
MarketAnnouncements        InvestorQuestions
PolicyDocuments            SecurityProfiles
FinancialStatements        MarketStatistics
TechnicalBars              CorporateActions
BoardDirectory             BoardConstituents
BoardMemberships           ResearchReports
ResearchDocuments          Consensus
TargetPrices               SemanticSearch
FundFlowSeries             BoardFlows
MarginData                 BlockTrades
HolderCounts               LockupEvents
DividendPlans              PostCloseFlows
NorthboundDaily            LimitPools
StrongStockReasons         DragonTiger
MarketDragonTiger          DragonTigerDiscovery
MarketRankings             MarketBreadth
Popularity                 ConceptHits
OptionData                 ProviderTopNRankings
InstrumentNews
```

所有方法都是只读 unary RPC。没有账户、资产、持仓、委托、撤单或成交写接口。

## 8. TDX 价格异动订阅

### 动态指定监控标的

`Subscribe.filter.instruments` 只过滤已经采集的消息，不改变 TDX 实际监控范围。控制方先
调用 `MarketEventService.SetWatchlist`，每次传入完整的新列表：

```proto
message SetWatchlistRequest {
  RequestContext context = 1;
  repeated string instruments = 2;
}
```

只接受非空、无重复的 `EQUITY:SH:600396`、`EQUITY:SZ:000001` 或
`EQUITY:BJ:430001` 形式。列表长度不能超过当前 Agent 在
`GetListenerStatus.maximum_watchlist_instruments` 中公布的上限。例如 JSON 请求：

```json
{
  "context": { "protocolVersion": 1, "requestId": "watchlist-20260815-1" },
  "instruments": ["EQUITY:SH:600396", "EQUITY:SZ:000001"]
}
```

成功响应的 `state` 为 `restarting` 或 `unchanged`。`restarting` 只表示命令已进入当前
Agent 的有界命令队列；调用方应轮询 `GetListenerStatus`，直到
`desired_watchlist_revision == applied_watchlist_revision` 且两份列表完全相等。列表改变会
重启固定 sibling monitor、创建新 generation 并清空旧窗口/重放，不能把旧 cursor 用于
新列表。没有活动 Agent 时返回 `UNAVAILABLE`，超上限或格式错误返回
`INVALID_ARGUMENT`。

动态控制是全局全量替换，不是追加，也不会按订阅者自动合并。多个控制方需要在调用方
侧协调；普通消费者只使用 `Subscribe.filter`。

动态列表只保存在当前 Server/Agent 运行期内。Server 与 Agent 同时重启后，会重新使用
部署参数文件中的初始 `--watchlist`；如果外部系统需要持久列表，应由它保存期望值，并在
连接恢复后再次调用 `SetWatchlist`，等待 desired/applied 状态一致。

2026-08-15 Windows 真实联调从初始单标的替换为
`EQUITY:SH:600396,EQUITY:SZ:000001`：响应为 `restarting`、desired/applied revision
均为 `1`，Agent 建立了新 generation，随后 Replay 分别收到两只标的各四条 observation。
这证明了实际采集范围发生了变化，不只是订阅端过滤。该次证据采集发生在生产准入前；
2026-08-15 后的新 generation 会把合格的 `observation` 和
`snapshot_observation` 标为 `ADMITTED`。

### 订阅事件

业务消费方调用 `MarketEventService.Subscribe`：

```proto
message SubscribeRequest {
  RequestContext context = 1;
  EventFilter filter = 2;
  EventCursor after = 3;
}
```

`EventFilter`：

- `instruments` 为空表示服务端授权范围内的全部标的；
- 非空时使用服务端发布的规范 instrument ID；
- `event_kinds` 可选择价格、成交量、成交额、状态和 reset 类事件；
- 未知值不能被当作通配符。

返回 `stream MarketEventEnvelope`：

```text
event_id
cursor.generation
cursor.sequence
event_kind
provider
instrument
observed_at
source_at
admission
payload
```

正式 TDX 原始事件包括当前价、累计成交量、累计成交额、昨收与 OHLC。`source_at`
为空表示厂商响应没有源时间，不能用 `observed_at` 代填。
`GetListenerStatus.admitted_event_families` 还列出三类 LocalAnalysis 异动；只有带合法
Core `AnomalyEvent` 的 trigger/rearm 才准入，预热、冷却与 reset 继续返回 `UNADMITTED`。

同一状态响应还提供 `replay_oldest`、当前 replay event/byte 数、活动订阅者数，以及进程
生命周期内 Agent connection/disconnection、已发布事件和 replay eviction 计数。这些字段在
既有 EventHub 状态锁中维护，不增加新锁、队列或 exporter；调用方可以用
`replay_oldest..latest` 判断当前可重放窗口，但仍必须按实际 Replay 结果处理 gap。

`analysis` 事件的 `observed_at` 来自监控器生成异动消息时绑定的触发观测时间，原始
payload 同时声明 `time_basis=local_observation_time`。Agent 接收时间和 gRPC 发送时间
都不会覆盖该值；`source_at` 继续为空。异动 frame 在顶层携带规范证券标识，订阅和
重放的 instrument filter 不依赖嵌套 payload 推断。

消费方必须持久化最后成功处理的 `generation + sequence`。generation 改变表示 TDX
重启、终端替换或服务明确重建连续性，不能把新旧 generation 拼成连续行情。

### 断线恢复

调用 `MarketEventService.Replay` 并传入最后已处理 cursor。重放是有界、同 generation、
best-effort：

- 返回成功：按 sequence 顺序处理；
- `OUT_OF_RANGE`：cursor 已早于重放窗口，调用方记录明确 gap；
- `FAILED_PRECONDITION`：generation 不匹配或连续性已重置；
- 不得把重放描述为 exactly-once 或 at-least-once。

### TDX 当前准入状态

TDX `price/cumulative_volume/cumulative_amount/previous_close/ohlc` 已为 `ADMITTED`，
精确单位分别是 CNY/share、lot、CNY 和 CNY/share。它们没有源时间，`source_at`
保持空；不能据此声称 tick freshness。`source_record_count` 仍不可用。消费端必须继续
按每条 envelope、事件类型和字段 admission marker 隔离。

服务端再次解析 admitted payload，只接受带匹配 instrument、schema、字段 admission marker
和实际值的 `observation`/`snapshot_observation`。Agent 试图提升 `analysis` 或不支持的
event kind 时会被拒绝，传输层不能自行扩大 repository admission。

2026-08-17 当前部署的双标的 production replay 验证了 admitted
`observation`、`snapshot_observation` 和有界 `analysis` 状态；Listener Status 返回
`agent_connected_production` 以及五个原始字段族和三个异动事件族。一次有界重放包含
173 条 admitted 事件和 3 条未准入预热状态，昨收/开/高/低分别观测为
16.99/16.44/17.28/16.43。

### 已接入的证券资料请求

`SecurityMetadata` 使用以下 canonical schema：

```text
schema = magic.market.security_metadata.request
data   = {"instruments":[{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}]}
```

腾讯来源覆盖 1..=50 个唯一沪深京股票。名称和 ST 标记来自源快照，板块为显式派生；
来源未证明的上市日期、涨跌停规则及规则版本保持 unavailable，因此该调用可能返回
`admission=ADMITTED` 且 `complete=false`。调用方必须保留字段级质量，不能把空字段补成
默认值。

`SecurityProfiles` 使用相同 instruments JSON，schema 为
`magic.market.security_profiles.request`。TDX 公共协议只覆盖 1..=8 个唯一沪深股票，返回
精确名称、可选财务包上市日和唯一 `公司概况` F10 原始行事实；不推断行业、总股本或
流通股本。F10 没有可信源时间，因此 `source_at` 为空。精确范围与实测证据见
[TDX 公共公司资料准入](tdx-public-security-profile.md)。

## 9. TDX Agent 接口

`TdxAgentService.OpenStream` 只供同仓库 Windows Agent 使用，普通业务系统不要调用。

```text
Windows TDX Agent --client stream--> gRPC Server
Windows TDX Agent <--server commands-- gRPC Server
```

第一条消息必须是 `AgentHello`，后续只能发送有序 Event 或 Heartbeat。服务端返回 Ack、
Stop 或严格类型化的只读 watchlist replacement。该命令只能携带 revision 和规范化股票
身份；协议没有 URL、阈值、下单、撤单或账户命令。

Agent 的 `--heartbeat-interval-ms` 必须为正；服务端的
`--agent-heartbeat-timeout-ms` 必须大于部署使用的心跳间隔。Agent 在没有事件时发送
带当前 generation/cursor 的 Heartbeat。服务端从 Hello 开始对每条后续消息执行该
截止时间；超时会原子移除 Agent session/命令通道并把监听状态改为 disconnected，避免
进程仍在但 monitor 已静默时继续接受 watchlist 命令。

Windows Agent 只启动同目录 `magic-market-monitor-server.exe`，并从同目录、最大
64 KiB 的 `magic-market-monitor-server.args.json` 读取 JSON 字符串数组参数；不搜索
`PATH`，也不接受 helper/TDX/17709 地址覆盖。Agent 到远程服务必须提供服务端 CA、
客户端证书和私钥；只有精确 loopback gRPC 地址允许明文。

## 10. 当前实现状态

- Protobuf/descriptor、60 个 unary RPC、health/capabilities、Bearer auth、远程 mTLS、
  blocking 调用隔离均已实现；
- 事件服务已实现严格 generation/sequence、同 generation 有界 replay、过滤和慢消费者
  显式终止；
- TDX Agent 双向流、空闲心跳、服务端存活截止时间、动态全量 watchlist replacement 和
  Windows 固定 sibling monitor 重启/转发已实现；五类本地终端字段和三类带证据的
  异动 trigger/rearm 进入生产事件流；
- unary registry 对 60 个操作逐项精确登记；除 `EconomicCalendar` 外，每个操作至少有一个证据支持的正式 handler；该日历操作因金十免费日历/API 于 2025-12-01 退役而 fail-closed，仅保留显式诊断路径；
  除既有 Tencent、Eastmoney、CNInfo、CFETS、FRED、SEC EDGAR、WallstreetCN、Jin10、
  HKEX、THS、State Council、iWencai 与官方 `HithinkFinance` 扶摇 API 外，也可精确选择
  TDX 公共协议、Sina、SSE、SZSE、
  CLS、ThePaper、XinhuaFinance、Yicai、SecuritiesTimes、NBS、PBC 与 WorldBank；
- GlobalNews 的财联社正式选择器为 `Cailianpress`，历史 `Cls` 作为兼容别名保留；
  两者返回的逐条 evidence provider 都必须是 `Cailianpress`；
- `InstrumentNews` 是 append-only 的第 55 个操作，只接受 Sina 已验证的沪深 A 股公司新闻
  合同；请求 schema 为 `magic.market.instrument_news.request`，记录 schema 复用
  `magic.market.news_item`；请求和记录版本均为 2，日期范围必须同时提供 start/end 或同时
  省略，并且必须携带调用方精确 `captured_through`；
- `IndexQuotes`、`IntradayShape`、`T0Evidence`、`OutcomeDailyBars` 和
  `UpperLimitPoolReview` 是 append-only 的第 56..60 个操作。其版本化请求/记录字段见
  [`grpc-derived-products.md`](grpc-derived-products.md)；`IndexQuotes` 已绑定腾讯六指数
  严格 freshness composition，`IntradayShape` 已绑定腾讯完整分钟序列确定性派生，
  `OutcomeDailyBars` 已绑定 TDX-only 精确 through 日线，`UpperLimitPoolReview` 已绑定东财
  同交易日四池原子组合；`T0Evidence` 正式读取 TDX Quote、盘口、日 K 和 5 分钟 K，
  v2 必须接收并逐条原样回显调用方的精确 `requested_at`，返回当前本地 `observed_at`、
  保留四份输入证据并在无公共源时间时保持 `source_at=null`；
- `MoneyFlows`、`FundFlowSeries` 已绑定东财公开资金流正式合同，`TechnicalBars` 已绑定
  Baidu 未复权源技术日线正式合同；`PostCloseFlows` 已绑定东财当前交易日 15:35 后的
  本地观察快照；`FuturesDelivery` 已绑定 CFFEX 官方固定交割日历，Baidu
  `HistoricalBars`、`MarketRankings` 仍登记显式 opt-in 诊断 handler；
  EMQuant `HistoricalBars` 已绑定沪深股票显式区间的完成日线生产合同；其 Quote、日内 K、
  盘口和资金流仍只作为显式诊断来源；配置
`EASTMONEY_API_KEY`（兼容别名 `MX_APIKEY`）后，东财妙想 `Auctions` 和
  `MarketBreadth` 登记已准入的窄版生产 handler；其 `FundFlowSeries` 和 `MoneyFlows`
  变体仍是显式诊断，要求 `preferred_provider=EastmoneyMiaoxiang` 且
  `allow_unadmitted=true`；
- 未配置东财妙想 Key 时，`Auctions` 和 `MarketBreadth` 仍在 I/O 前
  `UNIMPLEMENTED`；配置后也只返回源直接给出的部分字段，不用普通 Quote 冒充竞价，
  不把不完整家数统计提升为完整市场宽度；
- `preferred_provider` 非空时必须精确选择已登记来源；空值选择该操作第一个可用登记。
  当前不会在一次请求内部隐藏切源，上游失败会原样形成 typed gRPC error，调用方可根据
  capabilities 和业务路由策略发起有界重试；
- 当前 10 条未准入 Provider×operation 路径及可显式选择的准入 operation 路由见
  [`unadmitted-provider-routes.md`](unadmitted-provider-routes.md)。这些路由只表示相同
  operation 下的独立来源，不表示数据集或 Provider 等价；例如 NBS/PBC/WorldBank
  不能重标为 IMF，Hithink 竞价快照也不能用其它响应的日期补齐；
- FRED、SEC EDGAR、iWencai、`HithinkFinance` 和东财妙想还要求对应运行时环境身份；缺失时 capability 保留
  repository admission、但 `runtime_available=false`，请求会在 I/O 前失败。
- EMQuant 日线还要求官方 SDK、激活文件和有效账号权限。权限到期或不足时返回类型化
  unavailable 且无 records，不是 `ADMITTED` 空批次，也不会回退到其他 Provider。
- 2026-08-14 当前实例通过 `SemanticSearch` + `preferred_provider=Iwencai` 实测返回
  10 条 `Report` 记录；Key 只从服务进程环境加载，不进入请求、日志或证据。

EMQuant 生产日线请求必须使用 `schema=magic.market.historical_bars.request`、
`schema_version=1`、`preferred_provider=EmQuant`、`allow_unadmitted=false`，payload 示例：

```json
{
  "instrument": {"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
  "interval": "Day",
  "start": "2026-08-18",
  "end": "2026-08-20",
  "limit": 5
}
```

生产范围只包含沪深股票、显式 inclusive 起止日期、未复权完成 `csd` 日线且最多 800 条。
省略日期、其他周期、北交所/非股票、未完成当日空字段、部分响应或证据冲突都会整批失败。
响应顶层 `provider=EmQuant` 表示所选 SDK 接入；逐条日线证据保留真实
`provider=Eastmoney`、源日期、观测时间和批次 ID。

官方同花顺扶摇 Provider 的选择器是 `HithinkFinance`，运行时 Key 只从
`HITHINK_FINANCE_API_KEY` 加载。当前正式准入七项现有 operation，不增加或改写 Protobuf：

| operation | 精确范围 |
| --- | --- |
| `HistoricalBars` | 沪深北六位股票和标准 `.SH`/`.SZ` 指数最长十年、沪深 ETF 最长五年；`Day`、显式 inclusive 起止日期、未复权，完整响应校验后取最新 caller limit |
| `MarketStatistics` | 1..=100 个唯一沪深北股票；只映射 `pe_ttm`、`pe_mrq`、`pb_mrq`，负值和 `null` 保留 |
| `LimitPools` | 显式上海交易日的 `Upper`、`Lower`、`Broken`；取完并校验所有声明页后才应用 limit；`PreviousUpper` 不支持 |
| `Popularity` | 官方 `period=day` 24 小时热股榜，最多 100 条，保留排名、热度、排名变化和响应源时刻 |
| `FinancialStatements` | 1..=8 个唯一 A 股；`Income`/`Balance`/`CashFlow` 最近 20 个季度，逐条保留 `report_date_ms` 源证据和显式 `null` |
| `CorporateActions` | 单只 A 股、可选且不晚于当前上海日期的 inclusive 范围；只映射官方现金/送股每股条款和除权日；源未给批次时间时 `source_at=null` |
| `SecurityMetadata` | 1..=32 个 A 股、标准指数或场内基金；精确身份/名称/币种，未发布的板块/上市日/涨跌停规则保持缺失并标为 `Unavailable` |

此外，`Auctions` 已实现同花顺当前最终快照的显式诊断，但不属于上述七项生产准入。
请求必须使用 `schema=magic.market.hithink_current_auctions.request`、
`preferred_provider=HithinkFinance`、`allow_unadmitted=true`：

```json
{"instruments":[{"exchange":"Shanghai","code":"600519","asset_class":"Equity"}]}
```

服务端固定发送 `stage=final`，只接受实测的 `auction_phase=closed`、
`data_status=final` 和完整精确身份。响应中的竞价量单位“手”严格乘 100 写入“股”，成交价、
昨收、涨幅、成交额和量比按源语义映射。实测单一 `auction_unmatched` 可以为负，但源合同没有
定义其符号到 bid/ask 的映射，故只校验为有限值，两个方向字段均保持 `null`。
`auction_phase=closed,data_status=not_ready` 明确返回零 records 的
`provider_unavailable`（`retryable=true`），不会冒充 malformed response 或部分成功。
`data.timestamp` 是响应组装时间，只能写入 `observed_at`；源未提供交易日和
逐条源时刻，因此不得写入 `trading_date` 或 `source_at`，也不能用批次时间、本地时间或其他
Provider 补齐。该诊断不能替代 `magic.market.auctions.request` 的精确日期合同。
其 gRPC 外层固定为 `admission=UNADMITTED`、`complete=false` 并携带 blocker；records
仅供显式诊断消费，不能被客户端提升为生产成功。

2026-08-22 脱敏真实 record 示例（仅替换 `batch_id`）：

```json
{
  "instrument": {"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
  "name": "贵州茅台",
  "matched_price": 1291.5,
  "previous_close": 1291.5,
  "change_percent": {"value":0.0,"unit":"Percent"},
  "matched_quantity": 16700.0,
  "matched_amount": 21568050.0,
  "unmatched_bid_quantity": null,
  "unmatched_ask_quantity": null,
  "volume_ratio": {"value":0.4718,"unit":"Decimal"},
  "status": "Unavailable",
  "source_at": null,
  "observed_at": "unix-ms:1787388432543",
  "provider": "Tonghuashun",
  "batch_id": "HITHINK_AUCTION_REQUEST_ID"
}
```

例如扶摇未复权日线请求使用现有 v1 schema：

```json
{
  "instrument": {"exchange":"Beijing","code":"920403","asset_class":"Equity"},
  "interval": "Day",
  "start": "2026-08-18",
  "end": "2026-08-21",
  "limit": 10
}
```

显式日期涨停池请求为：

```json
{"kind":"Upper","trading_date":"2026-08-21","limit":10}
```

财务三表、公司行动和元数据继续使用现有 v1 业务 JSON：

```json
{"instruments":[{"exchange":"Shanghai","code":"600519","asset_class":"Equity"}],"kind":"Income"}
```

```json
{"instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},"start":"2025-01-01","end":"2026-08-21"}
```

```json
{"instruments":[
  {"exchange":"Shanghai","code":"600519","asset_class":"Equity"},
  {"exchange":"Shanghai","code":"000300","asset_class":"Index"},
  {"exchange":"Shanghai","code":"510300","asset_class":"Fund"}
]}
```

上述七项生产调用必须设置 `preferred_provider=HithinkFinance` 和
`allow_unadmitted=false`。响应顶层
`provider=HithinkFinance` 表示官方扶摇接入，当前 Core 记录 evidence 使用
`provider=Tonghuashun`；两者不是跨源拼接。日线逐条 `source_at` 是各自交易日；估值响应
时间只表示本批固定五项指标中的最新有效上游时间，因此只放在批次 provenance，不能复制
成每条估值记录的共同指标时刻。财务记录各自的 `source_at` 是原始
`report_date_ms`；批次时间是最新报告发布日期，不能覆盖较早报告。公司行动端点没有来源
时间，因此批次和逐条 `source_at` 都保持 `null`，不得用除权日或本地时间冒充。
`SecurityMetadata` 返回完整的“身份解析批次”，但记录状态为 `Unavailable`，因为板块、ST、
上市日和涨跌停规则并未由该端点发布。标准指数的 provider-native ticker（实测
`000300.SH` 对应 `1B0300`）不替换 Core 的精确 `thscode` 身份。

扶摇 Key 缺失或到期、认证/权限拒绝、限流、查询拒绝、
上游不可用和响应冲突都返回闭合 typed failure 和零 records，不回退 `magic-ths-rs` 网页源。
扶摇显式代码实时快照没有 source timestamp，因此不以 `HithinkFinance` 注册 handler。
集合竞价只注册上文的当前快照诊断，不注册生产 handler；客户端不得从本地时间或其他
Provider 补齐其交易日、`source_at` 或未匹配方向。

以下特定来源变体不是缺少 gRPC 方法，而是该来源的生产数据合同尚未满足。已有字段通过显式
诊断模式读取，缺失字段保留 `null`，但不会改变下表状态：

| 操作 | 当前阻塞原因 |
| --- | --- |
| `MarketRankings` | 诊断只读取首个有界来源页，并返回来源声明总数；不声称完整市场覆盖或源时间原子性 |
| `FundFlowSeries` / `MoneyFlows`（东财妙想） | 自然语言查询的结果基数、来源方法和串行稳定性仍未完成独立准入；不影响东财公开接口的正式资金流路由 |
| `Auctions`（同花顺扶摇） | 当前最终快照缺少精确交易日、方向化未匹配队列和 Provider 源时刻，只能使用 provider-specific 诊断 schema |

### 相关请求 schema

所有 payload `schema_version=1`：

| Operation | request schema | record schema |
| --- | --- | --- |
| `TechnicalBars` | `magic.market.technical_bars.request` (`BarsRequest`) | `magic.market.technical_bar` |
| `FundFlowSeries` | `magic.market.fund_flow_series.request` (`FundFlowRequest`) | `magic.market.fund_flow_point` |
| `MoneyFlows` | `magic.market.money_flows.request` (`{"instruments":[...]}`，精确 1 个) | `magic.market.money_flow` |
| `FuturesDelivery` | `magic.market.futures_delivery.request` (`FuturesDeliveryRequest`) | `magic.market.futures_delivery_event` |
| `PostCloseFlows` | `magic.market.post_close_flows.request` (`PostCloseFlowRequest`) | `magic.market.post_close_flow` |
| `MarketRankings` | `magic.market.market_rankings.request` (`{"kind":...,"limit":...}`) | `magic.market.market_ranking_diagnostic_entry` |
| `Auctions` / `EastmoneyMiaoxiang` | `magic.market.auctions.request` (`{"instrument":...,"trading_date":"YYYY-MM-DD"}`) | `magic.market.opening_auction_diagnostic` |
| `Auctions` / `HithinkFinance` diagnostic | `magic.market.hithink_current_auctions.request` (`{"instruments":[...]}`) | `magic.market.hithink_current_auction_snapshot` |
| `MarketBreadth` | `magic.market.market_breadth.request` (`{"source_date":"YYYY-MM-DD"}`) | `magic.market.market_breadth_diagnostic` |

例如技术日 K 诊断的业务 JSON 为：

```json
{"instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"interval":"Day","start":null,"end":null,"limit":20}
```

所有未准入诊断的外层 `QueryRequest` 都必须同时设置对应的
`preferred_provider` 和 `allow_unadmitted=true`。MA5/MA10/MA20 及资金流分档等源端未提供的可选字段保持
`null`，调用方不得补零。盘后资金诊断中的 `super_large_net`、`large_net` 以及来源缺失
字段同样保持 `null`；排行诊断同时返回 `reported_universe_size` 与 `fetched_count`，不得
把首个来源页解释为完整市场。

东财妙想资金流诊断必须设置 `preferred_provider=EastmoneyMiaoxiang` 和
`allow_unadmitted=true`。窄版集合竞价与市场宽度已独立准入，应使用同一精确 Provider 和
`allow_unadmitted=false`。服务端启动时检测 Key 只决定处理器是否运行时可用；Key 只放在
服务进程环境，绝不能放入 `QueryRequest`。例如开盘集合竞价请求：

```json
{"instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},"trading_date":"2026-08-14"}
```

返回记录中 `matched_quantity_shares` 和 `matched_amount_cny` 有源值，其余未证明竞价字段
为 `null`；这是证据边界明确的窄版已准入记录，不宣称完整 Level-2。市场宽度请求为
`{"source_date":"2026-08-14"}`，只消费五个已证明家数；`listed_total`、`coverage` 和
`maximum_source_skew_millis` 保持 `null`。

2026-08-15 当前工作站真实 gRPC 验证：

| Operation | 结果 | 观测摘要 |
| --- | --- | --- |
| `TechnicalBars` | 返回 1 条，`UNADMITTED` | 600396.SH，2026-08-14 未复权日 K，含 MA5/10/20 |
| `MoneyFlows` | 返回 1 条，`UNADMITTED` | 600396.SH，2026-08-14 五档资金净额；未使用 TDX 成交额冒充 |
| `MarketRankings` | 返回 2 条，`UNADMITTED` | 来源声明总数 5554、首屏抓取 100；两条源时间不同，明确非原子 |
| `PostCloseFlows` | 返回 2 条，`UNADMITTED` | 显式请求 2026-08-14；`super_large_net`/`large_net` 为 `null` |
| `FundFlowSeries` | 东财妙想返回记录，`UNADMITTED` | 600396.SH 日级五档净额；服务端有界截断源端多返回日期 |
| `FuturesDelivery` | 历史诊断曾返回 4 条，`UNADMITTED` | 该 2026-07 明文通知探测仅是历史证据；当前正式合同改用 CFFEX 固定官方交割日历 |
| `Auctions` | 东财妙想返回部分记录，`UNADMITTED` | 2026-08-14：开盘竞价成交量 2,951,900 股、成交额 53,665,542 元；其他字段为空 |
| `MarketBreadth` | 东财妙想返回部分记录，`UNADMITTED` | 2026-08-14：上涨 2400、下跌 2970、平盘 170、涨停 64、跌停 13；总数/覆盖率/偏差为空 |

上表是 2026-08-15 的历史诊断证据，不代表当前准入状态。2026-08-17 部署后再次通过
正式 gRPC 路径验证：`TechnicalBars` 由 Baidu 返回 600396.SH 当日未复权日 K 和
MA5/10/20，`MoneyFlows` 与 `FundFlowSeries` 由 Eastmoney 返回 600396.SH 当日及近三日
五档资金净额，三项响应均为 `ADMITTED`。本次本地观察时间变更完成源码与直接
composition 实测后，2026-08-18 更新的部署实例通过远程 mTLS + Bearer 再验证：能力
注册表为 60 项操作、56 项正式准入、4 项阻塞；正式 `T0Evidence` 返回
`complete=true`、`ADMITTED`、当前本地 `+08:00` `observed_at` 和 `source_at=null`。

同日 `PostCloseFlows` 完成 2 次 live 和 3 次串行 load：20/3 条结果均按来源主力净流入
排序，逐条 `source_at` 保持真实且日期为 2026-08-17，批次使用当前本地
`observed_at`，混合源时刻使批次 `source_at=null`。`T0Evidence` 完成 2 次 live 和 3 次
串行读取，每次返回 600396.SH 的 Quote、五档、20 根日 K、20 根 5 分钟 K 和四份输入
证据，结果 `complete=true`、`repository_admitted=true`，当前本地观察时刻带 `+08:00`，
公共源时间仍为 `null`。

跨日后的 2026-08-18 00 时段又通过同一远程正式 `PostCloseFlows` RPC 验证本地时间门：
当天尚未达到 15:35，服务明确拒绝请求并在 Provider I/O 前停止。它没有沿用前一交易日，
也没有把昨日证据冒充今日数据。2026-08-18 的远程成功样本只能在当天 15:35 后重测；
这不影响上一段 2026-08-17 已完成的 2 次 live 与 3 次串行 load 证据。

2026-08-17 的 `plaintext_http_diagnostic` 结果是历史诊断材料，不再描述当前发布合同。
当前 `FuturesDelivery`、`preferred_provider=Cffex` 在 `allow_unadmitted=false` 下正式
准入，读取 CFFEX 固定官方交割日历；客户端不得继续把旧 bundle 中的
`provider_unsupported` 解释为当前服务能力。

## 11. gRPC 错误处理

| gRPC code | 调用方行为 |
| --- | --- |
| `INVALID_ARGUMENT` | 修正请求/schema/cursor，不自动重试 |
| `UNAUTHENTICATED` | 刷新或更换凭据 |
| `PERMISSION_DENIED` | 停止该能力调用并联系授权方 |
| `UNIMPLEMENTED` | 能力未准入或不支持，不重试 |
| `RESOURCE_EXHAUSTED` | 按服务端策略退避；流消费者需记录 gap |
| `DEADLINE_EXCEEDED` | 有界退避重试，保留原 request_id |
| `UNAVAILABLE` | 有界指数退避，重新检查 health/capabilities |
| `FAILED_PRECONDITION` | 数据完整性/连续性失败，不能当空成功 |
| `INTERNAL` | 记录 request_id，停止无界重试 |

服务端把安全的 Protobuf `ErrorDetail` 编码在 trailing metadata
`magic-error-detail-bin` 中：request ID、operation、Provider、reason code、retryable、
admission，以及可选的 `evidence_code`、`evidence_field`、`record_index`。有序路由失败还
携带最多 16 个 `provider_attempts`，每项只有 ordinal、Provider、closed outcome/reason
code、retryable 和 terminal；上游自由文本、URL、响应体与凭据永不进入该数组。
证据拒绝固定使用 `reason_code=invalid_evidence`、`retryable=false`。GlobalNews 可用
`record_index` 定位被拒记录；Consensus 使用安全的结构化字段路径标识 Provider 响应中
发生冲突的 identity、年度、机构数、最小/均值/最大值或表结构，不回传敏感原文。
Provider 失败使用闭合的 `provider_authentication_rejected`、`provider_rate_limited`、
`provider_unavailable`、`external_query_rejected` 或 `provider_response_invalid`，保留安全
Provider 身份和确定的 retryable 标志。CLS 的原始 HTTP status、`errno`/`errmsg` 或解析
类别只进入服务端受限结构化日志，不进入 gRPC message 或 detail。
`records=[]` 或策略侧“扫描到 0 候选”只有在该策略声明的全部必需数据族都返回已准入、完整
或 verified-empty 时，才可解释为没有机会。任一 MoneyFlow、完成日线、T0Evidence 或其他
必需输入为 unavailable、partial、stale、UNADMITTED 或失败时，客户端必须记录
`incomplete_inputs`，不能把零候选提升为市场结论。
生产 stderr 日志以 `ts=<UTC RFC3339> level=<...> target=<...> event=<...>` 开头；TDX
兼容日志保留 `[E/W/I/D]` 级别标记并在其前面增加相同 UTC RFC3339 时间戳。成功行情轮询
和成功 unary 请求不逐条写日志，避免同步 I/O 进入热路径。
该自定义 detail 不占用标准 `grpc-status-details-bin`，因此 grpcurl 等标准客户端不会把
它误解为 `google.rpc.Status`。调用方不得依赖自然语言 message 做程序分支。

client-bundle `2026-08-19.2` 起，`manifest.sha256` 固定使用 ASCII+LF。Linux 可在
bundle 根目录运行 `sha256sum -c manifest.sha256`，macOS 可运行
`shasum -a 256 -c manifest.sha256`；两条命令不得要求调用方先转换换行符。
`2026-08-20.1` 修正 InstrumentNews 的合法 cutoff-empty 分类；protobuf wire 字段未变化。
`2026-08-22.1` 加入官方 `HithinkFinance` 四项生产 handler 和 EMQuant 正式日线；仍不改变
protobuf wire 字段，精确来源提交写入同一 bundle 的 `bundle-metadata.json`。
`2026-08-22.2` 把 `HithinkFinance` 扩展为七项生产 operation：日线新增标准指数/ETF，
并新增财务三表、公司行动和证券元数据；同时增加 provider-specific 当前最终集合竞价诊断，
不伪造交易日、`source_at` 或未匹配方向。该版本还修正财务 `data.timestamp` 与逐条发布日期
证据、指数 provider-native ticker 及炸板 `open_times=null` 语义，protobuf wire 字段仍未变化。
`2026-08-24.2` 同步当前 10 条未准入 Provider×operation 注册，保留 Jin10 日历与证券时报
新闻的显式 fail-closed 诊断，并把 TDX E2001..E2006 连接类错误稳定分类为可重试的
`provider_unavailable`。同版还允许 Sina InstrumentNews 的合法跨页时间窗口重叠：逐页端点仍须
单调不前移，合并结果在 cutoff 前按原始发布时间稳定排序；冲突重复或窗口整体前移仍整批
拒绝。TDX 失败连接会从池中移除，同一请求的重试不会重复选择已经失败的公共节点。
TDX 日内 K 线还会排除上游唯一、最新、同日且受限的未完成占位行
（包括午休前出现的 `13:00` 标签），并从同一 TDX 源补取一条更早的真实完成 K 线；不会
重写时间，也不会容忍多条、跨日、越界或无效未来行。Eastmoney 滚动财经页返回的官方
`fund.eastmoney.com/a/<纯数字>.html` 元数据已加入精确 allowlist，仍不抓取文章正文，也不
接受任意其它子域或后缀伪装。bundle 同时收录本文直接引用的 TDX
SecurityProfiles 与未准入路由合同，所有文件由同一 LF `manifest.sha256` 覆盖；protobuf wire
字段仍未变化。

`2026-08-27.1` 将 `T0Evidence` 升级到 v2 并要求 `requested_at`，在 Health 增加运行构建
身份，在安全错误 detail 增加完整有序 provider attempts。旧 T0 v1 请求明确拒绝，不由
服务端默认 capture 时刻。
`2026-08-27.2` 按深交所正式代码区间将 CLS `sz302132` 等 `300000..=309799` 关联身份识别
为创业板股票；`309800..=309999` 存托凭证仍因无对应 Core 资产类别而 fail-closed。

## 12. 客户端代码生成

### Python

```bash
python -m grpc_tools.protoc \
  -I crates/magic-market-grpc-contracts/proto \
  --python_out generated \
  --grpc_python_out generated \
  crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto
```

### Go

```bash
protoc \
  -I crates/magic-market-grpc-contracts/proto \
  --go_out generated --go_opt=paths=source_relative \
  --go-grpc_out generated --go-grpc_opt=paths=source_relative \
  crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto
```

Go 项目正式接入前可在自己的 Proto 镜像中补 `go_package` 映射，但不得修改字段号、
枚举值或 service/method 名称。

### Rust

使用 `tonic-prost-build` 编译同一 Proto，或直接依赖同版本
`magic-market-grpc-contracts` crate。禁止从服务端内部 crate 引用业务实现。

## 13. 联调检查表

- [ ] Proto 文件摘要与服务端发布版本一致。
- [ ] `protocol_version=1`，request_id 非空且可检索。
- [ ] 启动先调用 GetHealth 和 GetCapabilities。
- [ ] 比较 GetHealth.build_identity 与本次 bundle/发布制品的 descriptor 和二进制摘要。
- [ ] 远程连接验证 TLS hostname 和 CA。
- [ ] Authorization 只在 metadata 中注入。
- [ ] 为 unary 和 stream 分别设置客户端 deadline/keepalive。
- [ ] 不把 UNADMITTED、partial、缺 source_at 当作生产成功。
- [ ] 持久化 TDX generation/sequence，并处理 gap/reset。
- [ ] 对 RESOURCE_EXHAUSTED/UNAVAILABLE 使用有界退避。
- [ ] 采集 GetHealth/GetListenerStatus 聚合指标并按进程重启重置处理。
- [ ] 日志不输出 Token、完整敏感 payload 或上游凭据。

## 14. 服务端发布时需要交付给对接方

发布者使用 `tools/docs/build_client_bundle.ps1` 从同一工作树复制 `market.proto`、本文、
`grpc-derived-products.md`、`tdx-public-security-profile.md` 和
`unadmitted-provider-routes.md`。脚本拒绝 MarketDataService RPC 数不是 60 的 proto、拒绝
bundle 内任一 Markdown 相对链接缺失，并生成 `bundle-metadata.json` 与
`manifest.sha256`；对接方必须同时校验 bundle version、source commit 和文件摘要，不能
混用不同提交的“最新版”文件。

1. `market.proto` 和 descriptor set 摘要；
2. 服务地址、TLS CA、认证材料；
3. 服务端消息/并发/流/重放限制；
4. 已准入 capability 快照与精确 scope；
5. 每个已启用方法的 canonical request/record schema fixture；
6. TDX 各原始/异动 family 的独立 admission 状态；
7. 版本升级和字段废弃通知周期。
