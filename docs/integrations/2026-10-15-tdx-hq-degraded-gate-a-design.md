# Gate A 设计：TDX hq 降级状态的显式广播（2026-10-15）

状态：待批准（决策点见 §6，按编号拍板后进入 Gate B）
问题 op：无新增 op；复用现有 `SubscribeMarketEvents` 的 server-internal 通道
（零 proto、零合同、零 bundle 变化）

## 1. 问题

TDX hq 握手层故障（2026-10-14 13:59 → 2026-10-15 开盘，见当日故障记录）期间，
服务端按请求自动重连并自愈，失败只落日志。下游拿到的只是"该请求失败/空数据"，
**无法区分"hq 路由故障"与"真的没有数据"**——竞价量能链路 date_pool_rows=0、
榜空、P-02 不发送，没有结构化信号说明原因；人工排查数小时。

## 2. 已核实的现状（全部本仓实测）

- 客户端自愈已存在：`connect_to_any` 按请求重连；实测旧进程无需重启即恢复
  （10:18 Tdx 直连成功）。因此本设计**不做**任何重试/探测加压。
- 逐台失败详情本次已补（f1bbad6：attempted/blocked_skipped/last_error 摘要 +
  30 分钟 TTL 黑名单）——观测面改善，但只到服务端日志。
- 事件旁路已具备 server-internal 通道（38edeee：独立序列域、固定
  generation=server-internal、仅直播不可回放）——`t0_evidence_degraded` 为先例，
  模式可直接复用（evidence-gate-remediation 参考文档）。
- 受影响的链路：所有以 Tdx 为唯一/首选源的 op（整市场 hq 衍生品、T0Evidence、
  首选 Tdx 的 RealtimeQuotes）。

## 3. 拟议机制（推荐组合，待 §6 拍板）

- **检测点**：grpc-server 的 TDX 请求执行路径（app.rs 的 provider_error 映射
  层）——connect-class 失败计数（连续 N 次失败进入 degraded；首个成功离开）。
- **状态**：app 内共享小状态机 `hq_health { consecutive_failures, degraded: bool,
  last_error, last_emit: Instant }`；防抖冷却 ≥5 分钟（同一状态期内不重复发）。
- **事件**：进入 degraded 发一次 `tdx_hq_degraded`；恢复（首个成功）发一次
  `tdx_hq_recovered`。均走 `publish_local` 同构通道（generation=server-internal、
  仅直播、结构化日志同旁路）：
  - `tdx.hq_degradation.v1`：consecutive_failures、window_ms、last_error、
    affected_hint（如 "tdx-only ops"）
  - `tdx.hq_recovered.v1`：outage_ms、last_error
- 不改 proto / 不改 contract_sha256 / 不重建 bundle。

## 4. 被否决的替代方案

- **客户端（magic-tdx-rs）内周期探测并广播**：客户端无 hub 访问；且本事故性质
  是限流，后台加压有恶化风险（FD 风暴教训）。否决。
- **GetHealth 加字段**：proto 冻结，否决。
- **每个失败请求同步发事件**：30s×N op 的事件风暴。否决（状态机+冷却替代）。

## 5. 影响面与风险

- 爆炸半径：仅 `magic-market-grpc-server`（app.rs + 复用 events.rs 的
  publish_local；38edeee 已保证不干扰 agent 序列域）。
- 对外：订阅方新增两种 event_kind（EventFilter 自选，无破坏性）；无 wire 变化。
- 测试：状态机单元测试（N 次失败→一次事件；恢复→一次事件；冷却抑制重复）；
  端到端：失败路径→直播订阅者收到 degraded→成功→recovered。
- 运维入口（已在 TDX_CAPABILITIES 落档）：live_probe / e2103_server_walk /
  probe_servers()，本设计不重复造工具。

## 6. 待批准人拍板（按编号回答）

拍板结果（2026-10-15）：D1a / D2（N=3，冷却 5 分钟）/ D3a / D4（独立提交，随批部署）。以下原文保留。

- **D1 触发语义**：(a) 请求失败驱动（荐：真实流量、零额外探测、天然限流）；
  或 (b) 周期健康探测（独立于请求，但引入后台流量——与事故性质相悖）。
- **D2 阈值与冷却**：N=3 次连续失败进 degraded、冷却 5 分钟（荐）；或指定数值。
- **D3 恢复语义**：(a) 首个成功即发 recovered（荐，配合事件时间戳足够判别）；
  或 (b) 成功稳定 M 分钟后再发。
- **D4 实施与批次**：单独提交、随后续部署批次一起上线（荐；避免为观测件单独
  消耗一次 Mac 重钉）。

## 7. 未验证项（不写成结论）

- grpc-server 内 connect-class 失败的准确判定位置（provider_error 映射能否区分
  connect-class 与数据类错误）——Gate B 实施前逐行确认。
- Mac 侧对新增 event_kind 的消费意愿（订阅方自行过滤；发不发是服务端的事）。

## 8. 实施记录（2026-10-15，已实现）

- 判定点：`query_inner` 单一执行漏斗——失败经 `tdx_hq_connect_reason` 过滤
  （`ServiceError::Unavailable` 且 reason 含五个 TdxError 连接类签名：
  "Connection error:" / "Connection timeout" / "Setup handshake failed:" /
  "Server disconnected" / "Retry exhausted after"——即 `map_tdx_error` 路由到
  `unavailable()` 的稳定形态）；成功则以 `selected_provider == "Tdx"` 为恢复信号。
- 状态机：`HqHealth`（consecutive_failures / first_failure_at / degraded /
  degraded_since / last_error / last_degraded_emit）；N=3 进 degraded；冷却期内
  再降级只记日志（`tdx_hq_degraded_suppressed`）不重播；非 TDX 失败既不计入也
  不打断连败计数。
- 事件：`tdx_hq_degraded` / `tdx_hq_recovered` 走 `publish_local`
  （generation=server-internal、仅直播）；instrument="MARKET"；payload
  `tdx.hq_degradation.v1`（consecutive_failures / window_ms / last_error /
  affected_hint）与 `tdx.hq_recovered.v1`（outage_ms / last_error）。
- 测试：签名判定窄集、阈值/恢复/重置、冷却抑制（3 项，全绿）。
- 未做（记录）：端到端故障注入（写实 TDX 全断需 mock 网关或真实故障）；部署后
  以真实请求路径验证事件投递。
