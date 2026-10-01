# iWencai News 频道用于科技新闻搜索的准入调查（2026-09-25）

## 结论

**iWencai `News` 频道在当前运行实例中确实能对部分 Rubin／Muse 查询返回候选，但还不能认定为本项目已获明确授权、已完成频道级准入、可稳定检索科技新闻的来源。** Core 类型和 Provider 实现允许构造 `News` 请求；仓库登记与历史正式准入实测只证实了 **`Report` 频道**。本次只读诊断新增了 News 实际响应证据，同时发现两个别名查询因上游返回 `http://` 链接而整批失败，另有明显不相关候选。一次 HTTP/gRPC 成功不替代账号权限、用途许可和可用性验证。来源：[Core 频道与请求类型](../../crates/magic-market-core/src/research.rs)、[Provider 请求构造](../../crates/magic-iwencai-rs/src/lib.rs)、[准入说明](../integrations/iwencai-api.md)。

| 层次 | 当前可证明的状态 | 还缺什么 |
| --- | --- | --- |
| 接口/代码字段 | `SemanticChannel::News` 存在，Provider 将其序列化为 `channels: ["news"]`；`query` 非空，Provider 限制 `size ≤ 50`。 | 这只证明客户端会发请求，不能证明上游账号有 News 权限或返回 News 内容。 |
| 账号/API 授权 | 2026-08-14 的正式准入实测范围是 `Report`。本次通过当前运行实例的配置身份取得部分 News 响应，证明服务端当时接受若干请求；尚无账号 News 权限、技能标识及用途许可的官方确认。 | 从账号授权资料或官方支持确认 News 权限、允许的技能标识、配额、展示/保存条件。 |
| 仓库准入 | [`admissions.tsv`](../integrations/admissions.tsv) 将总能力 `SEMANTIC_SEARCH_ADMITTED` 记为 `true`，但其所指向的[接入说明](../integrations/iwencai-api.md)明确把准入限定为精确的 `Report` 频道、非空查询、limit ≤ 50。 | 为 News 频道单独完成 Gate A–D 和可执行的频道级准入约束，不能借用 Report 的证据。 |
| 科技新闻覆盖 | `NVIDIA Rubin`、`Meta Muse`、`Muse Spark` 均返回 10 条候选；`英伟达 Rubin`、`Vera Rubin` 因上游 `http://` 链接触发整批拒绝。成功批次含部分相关报道，也有不相关候选；前 10 条没有命中本报告所列 NVIDIA／Meta 官方原文。 | 核实账号/来源范围，修正无效链接的安全处理合同，并用固定真值样本评估召回、准确率与别名一致性。 |

## 可核查的仓库证据

1. [`SemanticChannel`](../../crates/magic-market-core/src/research.rs)（约 620–625 行）含 `Report`、`News`、`Announcement`、`General`。[`SemanticSearchRequest`](../../crates/magic-market-core/src/research.rs)（约 707–744 行）只提供查询词、单一频道、上限，没有日期、页码或来源筛选。Core 上限为 100，Provider 再缩小到 50。[`IwencaiClient::search`](../../crates/magic-iwencai-rs/src/lib.rs)（约 241–277 行）。
2. Provider 的 [`channel_name` 与 `build_request`](../../crates/magic-iwencai-rs/src/lib.rs)（约 389–430 行）确实把 `News` 编码为 `news` 并发送到固定 HTTPS 搜索端点。但是请求头**对所有频道均固定为** `X-Claw-Skill-Id: report-search`、`X-Claw-Skill-Version: 2.0.0`；代码没有 News 专属的技能标识或授权判断。因此必须向官方核实在 News 查询时该标识是否合规，不能自行推断其可复用。
3. [`admissions.tsv`](../integrations/admissions.tsv) 第 9 行只记录整个 `SEMANTIC_SEARCH_ADMITTED`；其引用的[`iwencai-api.md`](../integrations/iwencai-api.md) 第 34–37 行明确说准入的是 `Report` 频道，并强调 fixture 或仅有 Key 不会扩展准入。[gRPC 外部接口文档](../integrations/grpc-external-api.md)第 544–545 行记载的线上 10 条记录也是 `Report`。两个官方 live/load 示例都硬编码 `SemanticChannel::Report`：[live_probe](../../crates/magic-iwencai-rs/examples/live_probe.rs)、[load_probe](../../crates/magic-iwencai-rs/examples/load_probe.rs)。
4. 生产 [`register_iwencai`](../../crates/magic-market-composition/src/grpc_production.rs)（约 2659–2699 行）只检查总布尔值与运行时 Key，之后对 `SemanticSearchRequest` 直接转发；当前看不到频道级准入门。这是**实现边界与文档中精确准入边界不一致**的风险，不是 News 已获准入的证据。若服务端配置了 Key，客户端可能提交 News 请求；其结果应按未验证路径处理。
5. [`parse_document`](../../crates/magic-iwencai-rs/src/lib.rs)（约 511–559 行）把**请求频道**赋给返回文档，未从上游逐条结果独立验证频道；`parse_response`（约 463–485 行）把成功但空数组视为协议错误。News 验收因此需人工检查原始响应所属类别和真实“无命中”语义，不能只看规范化结果中的 `channel=News`。

## 2026-09-25 有界运行诊断

主代理经本地现有 gRPC 服务以 `preferred_provider=Iwencai`、`SemanticSearch` 请求 schema v1、`channel=News`、`limit=10` 做五次只读诊断；没有修改账号、准入、合同或服务数据，也没有保存上游原始响应。以下是当次会话的请求/响应摘要，不是可复现的全量覆盖率或商业使用授权证明。运行能力目录同时显示 `SemanticSearch/Iwencai` 为仓库已准入且运行可用，但目录只按操作登记，未显示频道级范围。[生产注册](../../crates/magic-market-composition/src/grpc_production.rs#L2659-L2699)、[准入范围说明](../integrations/iwencai-api.md#L34-L37)

| 查询 | 当次结果 | 可支持的判断 |
| --- | --- | --- |
| `NVIDIA Rubin` | 完整返回 10 条；有《英伟达推出新一代人工智能平台 Vera Rubin》等相关新闻，也有标题只谈 PCB、其他芯片或 2025 年 GTC 的候选。 | 英文词可以召回部分 Rubin 报道；前 10 条未见 NVIDIA 官方原文，且结果不能不经相关性过滤直接展示为“Rubin 新闻”。 |
| `英伟达 Rubin` | `FailedPrecondition`；一条上游结果 URL 为 `http://finance.sina.com.cn/...`，违反本项目 `HttpsUrl` 合同，整批无记录。 | 中文别名这次**未形成可用结果**；不是“没有新闻”。 |
| `Vera Rubin` | `FailedPrecondition`；一条上游结果 URL 为 `http://stock.finance.sina.com.cn/...`，同样整批无记录。 | 另一个英文别名也未形成可用结果；需要调查上游 URL 与严格合同如何安全兼容，不能悄悄放宽 HTTPS。 |
| `Meta Muse` | 完整返回 10 条；有《Meta 正式推出个人 AI 智能体 Muse》等候选。 | 能召回部分 Muse 智能体报道，但前 10 条未见 Meta 官方原文，且 Muse Spark 是另一个对象。 |
| `Muse Spark` | 完整返回 10 条；仅少数标题直接涉及 Meta Muse Spark，其他结果明显偏离，例如游戏盘点、能源新闻。 | 该模型主题在此查询下噪声较高，不能用“返回 10 条”代替 10 条相关命中。 |

生产实现把所请求的频道写到规范化文档中，尚未逐条证明上游结果真的属于 News；本次也只保存了标题、URL、时间等摘要，没有采集原始业务状态或账户授权页面。因此上表只能证明当前配置身份下的**部分实际返回与两次合同失败**。[解析与频道赋值](../../crates/magic-iwencai-rs/src/lib.rs#L463-L559)

## Rubin／Muse 的验收样例

这些是**事件真值锚点，不是 iWencai 已覆盖的证据**：NVIDIA 官方于 2026-01-05 发布 [Rubin 平台新闻稿](https://nvidianews.nvidia.com/news/rubin-platform-ai-supercomputer)，于 2026-06-22 发布 [Vera Rubin 科学计算新闻稿](https://nvidianews.nvidia.com/news/nvidia-vera-rubin-delivers-world-class-supercomputers-for-science)；Meta 官方于 2026-04-08 发布 [Muse Spark 模型公告](https://about.fb.com/news/2026/04/introducing-muse-spark-meta-superintelligence-labs/)，于 2026-09-08 发布 [Muse 智能体公告](https://about.fb.com/news/2026/09/introducing-muse-personal-ai-agent/)。应分别判定 **Rubin 平台、Muse Spark 模型、Muse 智能体**，不要把三个不同事件混成一个命中。

| 事件 | 建议的有界查询（每次 `channel=News`、`size=10`） | 判定为相关的最低条件 |
| --- | --- | --- |
| NVIDIA Rubin 平台 | `NVIDIA Rubin`、`英伟达 Rubin`、`英伟达 鲁宾` | 标题/摘要明确关联 NVIDIA 与 Rubin 平台或 Vera Rubin GPU，原文 URL 可打开，发布者与发布时间可核验。单独出现天文学家 Rubin 或普通人名不算。 |
| Meta Muse Spark | `Meta Muse Spark`、`Meta Muse Spark 模型` | 明确指向 Meta 的 Muse Spark 模型及其 2026-04-08 发布/后续报道；泛指 Muse 艺术、Muse Image 或 Muse 智能体不算此事件。 |
| Meta Muse 智能体 | `Meta Muse 个人 AI 智能体`、`Meta Muse agent` | 明确指向 Meta 于 2026-09 发布的 Muse 个人智能体；Muse Spark 模型报道不能代替。 |

建议固定测试日期、账号权限和请求/响应摘要（脱敏），对上述三项事件各取**前 10 条**，保留状态码、上游业务状态、可核验 URL、标题、摘要、来源、源发布时间、查询时间、返回数量和相关性判定。每项事件至少一种中英文别名在前 10 条中出现一条可核验相关新闻，才可说“这些样例可查到”；这仍只证明样例覆盖，**不证明全量科技新闻覆盖、时效 SLA 或历史检索完整性**。空结果、401/403、上游业务拒绝、协议解析失败必须分别记录，不能混称“没有新闻”。仓库现有请求也无日期/分页参数，单次 `size=10` 无法证明历史查全。[请求与响应处理](../../crates/magic-iwencai-rs/src/lib.rs)。

## 下一步判定门槛

1. 先向 iWencai 官方或账号管理方确认 News 频道和实际请求头技能标识的授权、调用配额、返回字段及内容再展示/保存许可。Report Key 曾成功不等于 News 权限存在；News 返回 HTTP 200 也不能代替书面/账号范围证据。[现有鉴权说明](../integrations/iwencai-api.md)。
2. 在明确授权的身份下补做**只读、低频、有限次数**的 News 正式验收，按上表记录 Rubin/Muse 的上游原始分类、真实空结果与相关性；特别复核两次 `http://` URL 导致的原子拒绝。不要打印 Key，不从浏览器 Cookie 取凭证，也不以 fixture 代替 live。现有 Provider 生产请求有串行与至少 1 秒间隔限制。[客户端实现](../../crates/magic-iwencai-rs/src/lib.rs)、[接入说明](../integrations/iwencai-api.md)。
3. 若权限与样例覆盖都成立，先按[工程 Gate A–D](../ENGINEERING_RULES.md)设计**频道级**准入、请求标识及“无结果”合同，再修改实现与登记，并验证兼容性、测试、格式、Clippy、文档和合规。若任一门槛不成立，仍需寻找获得检索/保存授权的其他资讯来源，不能把未验证的 News 路由宣称为已上线搜索。

## 本次调查边界

本文件结合静态代码、仓库准入、官方事件公告及本地运行实例的五次有界 News 诊断；主代理通过本地既有服务令牌访问 gRPC，令牌未输出或写入报告，未修改代码、合同、准入登记。上游公开页面是否有更细的 News API 文档、当前账号在许可和技能标识意义上的 News 权限、原始结果分类及长期质量仍待官方资料或正式准入验收补证。
