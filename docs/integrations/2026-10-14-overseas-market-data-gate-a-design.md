# Gate A 设计：海外市场数据（美股 / 港股 / 日股 / 韩股）（2026-10-14）

状态：待批准（决策点见 §8，按编号拍板后进入 Gate B）
问题 op：无。本设计**不选市场、不接来源、不修改任何 op、不改 proto**。§12.3 说明为什么
"只加一个指数点位"这条看似便宜的路本身就是一个需要批准人拍板的决定。

## 1. 问题

第九个问题的原始表述是"还有美股、港股、韩股、日股的数据也是需要的 / 这块怎么考虑"。

直觉读法是"缺四个数据源，去接四个源"。**核查后这个读法不成立**：仓库缺的不是源，是**身份**。
A 股以外的"市场"这个概念在 Core 的类型系统里**不存在**（`Exchange` 只有三个变体，§3.1），
因此无论接哪个源，海外个股都**无处安放**。

这与第六个问题（期货 / 可转债）是同一个病，但半径大一个数量级：

| | 第六个问题 | 第九个问题 |
| --- | --- | --- |
| 缺的是 | `AssetClass::Future` 一个变体 | **整个 venue 维度** |
| 编译期强制的改点 | 1 处穷尽 `match` | 23 处穷尽 + ~30 处**静默** + 13 处解析 + 1 个平行枚举 |
| 现有可用源 | 有（TDX / CFFEX / 东财） | **没有**（§5） |

## 2. 今天的实际覆盖（逐项核实）

| 市场 | 已有什么 | 没有什么 |
| --- | --- | --- |
| **美股** | 3 个**指数点位**（`int_dji` / `int_nasdaq` / `int_sp500`）；SEC EDGAR **公告元数据** | 个股行情、财报正文、XBRL |
| **港股** | 恒生指数点位 1 个（`int_hangseng`） | **港股个股 0**；唯一 HKEX 通道是**北向资金**（外资流入 A 股，见 §6） |
| **日股** | 日经 225 点位 1 个（`int_nikkei`） | 个股；任何日本 provider |
| **韩股** | **0**，连指数都没有 | 全部 |
| 外汇 | 8 个汇率对（`fx_s*`） | 无 |
| 宏观 | FRED、World Bank（非行情） | IMF（blocked，见第八个问题设计） |

**一句话：海外只有 6 个指数点位 + 8 个汇率，海外个股一只都没有。**

核实依据：

- 指数与汇率的全集是**闭枚举**：`GlobalIndexCode`（`crates/magic-market-core/src/global.rs:7-14`）
  6 个变体、`FxPair`（`:69-78`）8 个变体，无第 7 个指数、无第 9 个汇率对。
- `crates/` 内 `kospi|krx|kosdaq|dax|taiwan` **零命中**；"Korea" 只命中 `magic-yonhap-rs`
  的新闻频道（`crates/magic-yonhap-rs/src/lib.rs:38-39`）与一个新闻时间戳偏移（`:741-743`）。
- SEC 侧自述边界：`magic-sec-rs/src/lib.rs:122-128` 返回
  `FilingCapabilities { filing_metadata: FILING_METADATA_ADMITTED, filing_documents: false, xbrl_facts: false }`。
- `Cargo` 侧：`ProviderId`（`crates/magic-market-core/src/provider.rs:37-71`）32 个变体中有
  `Hkex`、`SecEdgar`，**没有任何海外证券行情 provider**。唯一的"有枚举无 crate"成员是
  `Baostock`（`crates/` 下无 `magic-baostock-rs`）；`Imf` 有真实 crate 但 `admitted=false`。
- `docs/integrations/sina-web.md:35` 的准入范围自述："精确请求、精确返回；**6 个已验证指数与
  8 个已验证汇率对**"。

## 3. 三个结构性障碍

### 3.1 身份层：`Exchange` 只有沪/深/京，且有第二个平行枚举

```rust
// crates/magic-market-core/src/instrument.rs:5-9
pub enum Exchange { Shanghai, Shenzhen, Beijing }
```

`InstrumentId { exchange, code, asset_class }`（`instrument.rs:20-25`）的 `new` 只校验
"非空 + 无控制字符"（`:33-41`）——**类型本身不阻止任何东西**，阻止来自上层校验器（§3.2）
和下游每一处假设。

**第二个平行枚举**：`SourceExchange`（`crates/magic-tdx-local-rs/src/protocol.rs:440-444`），
注释"Explicit exchange identity supplied by the bridge"，同样三个变体。它有独立的穷尽 match
（`loopback.rs:71`、`magic-market-monitor-server/src/analysis.rs:437`）与解析入口
（`magic-market-monitor-server/src/config.rs:449-459`）。**加一个海外 venue 要改两个枚举。**

**加一个 `Exchange` 变体的代价（穷举统计，抽样已核实）**：

| 类别 | 数量 | 编译器会提醒吗 |
| --- | --- | --- |
| 穷尽 `match`（scrutinee = Core `Exchange`） | **23** 处生产 src + 3 处 example | 会（硬报错） |
| **静默 fallthrough**（`== Beijing` 特判、`matches!` 白名单、`!= 某venue` 守卫、`HashSet::from([沪,深,京])`、首字母推导 venue） | **~30** 处 | **不会** |
| 字符串 → `Exchange` 解析 | **13** 处（`"SH"/"SZ"/"BJ"`、`"SHANGHAI"`、SECUCODE 后缀、`pageColumn`、thscode 后缀…） | 不会 |
| 平行枚举 `SourceExchange` | 2 处穷尽 + 1 处解析 | 会 |

代表性穷尽点：`market_event.rs:1285-1291`（`exchange_name`，已核实为 3 臂穷尽）、
`magic-tencent-rs/src/lib.rs:265/319/374`、`magic-tdx-rs/src/service/mod.rs:23`、
`magic-eastmoney-rs/src/lib.rs:356`、`magic-hithink-rs/src/lib.rs:1421`。
代表性**静默**点：`magic-eastmoney-rs/src/market_rankings.rs:633`
（`HashSet::from([Shanghai, Shenzhen, Beijing])`——第 4 个 venue 被**静默排除**）、
`magic-hithink-rs/src/lib.rs:891`（`matches!` 白名单——**静默拒绝**）、
`magic-market-composition/src/derived_products.rs:49`（`== Beijing` 特判——**静默放行**）。

> **关键成本不在 23，在 ~30。** 编译器只保护 23 处；其余 30 处会编译通过，然后静默地把海外
> 标的塞进 A 股专用逻辑，或静默拒绝。这是本设计主张"先定身份层再做任何市场"的核心理由。

`#[derive(Serialize, Deserialize)]`（`instrument.rs:4`）**不提供额外保护**：serde 对新增变体
只是多一个合法字符串。

### 3.2 代码格式：全仓把"六位数字"当证券身份

派生品契约里，这个约束被写死并且**函数名就直接叫 A 股**：

```rust
// crates/magic-market-composition/src/derived_products.rs:1126-1145
fn validate_a_share_equity(instrument: &InstrumentId) -> ... {
    if instrument.asset_class() != AssetClass::Equity { /* "is not an A-share equity identity" */ }
    validate_six_digit_code(instrument)
}
fn validate_six_digit_code(instrument: &InstrumentId) -> ... {
    if instrument.code().len() != 6 || !instrument.code().bytes().all(|b| b.is_ascii_digit()) {
        /* "{code} must be a six-digit market code" */
    }
}
```

调用点：`IndexQuotesRequest::new`（`derived_products.rs:52`）与 `validate_a_share_equity`（`:1133`）。

provider 侧同样硬：

```rust
// crates/magic-hithink-rs/src/lib.rs:1429-1435
if ticker.len() != 6
    || !ticker.bytes().all(|b| b.is_ascii_digit())
    || thscode.len() != 9
    || thscode.as_bytes().get(6) != Some(&b'.')
    || !thscode.starts_with(ticker)
{ /* "malformed or contradictory thscode" */ }
```

Sina 的快照解析器要求符号键是 `hq_str_` + **恰好 8 字符** + 前两位 `sh|sz|bj`：

```rust
// crates/magic-sina-rs/src/lib.rs:377-385
.filter(|value| {
    value.len() == 8
        && matches!(&value[0..2], "sh" | "sz" | "bj")
        && value[2..].bytes().all(|byte| byte.is_ascii_digit())
})
```

**结果**：`AAPL`、`0700.HK`、`7203.T`、`^GSPC` **在每一个解析器上都会硬失败**。venue 普遍由
六位代码的**首位数字**反推（`magic-eastmoney-rs/src/lib.rs:441-452`、
`magic-cninfo-rs/src/lib.rs:802-813`、`magic-baidu-rs/src/lib.rs:278-288`、
`magic-ths-rs/src/lib.rs:1151-1161`）。全仓约有 **30 处**六位数字检查分布在 **23 个 src 文件**里。

> 注意：这不是"少写一个分支"。**"证券代码"在本仓的类型系统里被隐含地定义为"六位十进制
> 数字"**，只是从未被写成一个类型，所以无法在一处修改。

### 3.3 币种与时间：`Price` 没有币种，海外标的盖中国时间戳

`Price` / `Quantity` / `Money` 是同一个宏生成的 **裸 f64 newtype**，**没有单位、没有币种**：

```rust
// crates/magic-market-core/src/value.rs:40-47
finite_type!(Price,    "price",    |v: f64| v > 0.0, "must be positive");
finite_type!(Quantity, "quantity", |v: f64| v >= 0.0, "must be non-negative");
finite_type!(Money,    "money",    |_v: f64| true,   "must be finite");
```

**正确的币种类型是存在的，但没有接到任何价格上**：

```rust
// crates/magic-market-core/src/reference_data.rs:251-254
/// Checked three-letter currency label.
pub struct CurrencyCode(String);
```

它只被 `OfficialFxFixingIdentity { provider, base: CurrencyCode, quote: CurrencyCode }`
（`reference_data.rs:289-294`）使用——即 **CFETS 官方汇率**这一条路径。`GlobalIndexQuote.value`
（`global.rs:48-55`）与 `FxQuote.rate`（`:112-119`）都是无币种的 `Price`。

**后果：混币种会静默通过。** 全仓唯一的币种守卫是 hithink 财报里一句字符串比较
（`crates/magic-hithink-rs/src/financials.rs:286-290`，"A-share financial statement currency
must be CNY"），且是 provider 私有。Core 层的金额算术已经与币种无关
（例：`magic-market-composition/src/grpc_production.rs:3462-3463`）。`Money` 的 CNY 语义
只是文档约定（`magic-market-core/src/capital.rs:249-250`）。

**时间**：无时区库依赖（全仓无 `chrono` / `chrono-tz`，只有 `time`），偏移是整数字面量。
`magic-market-core/src/time.rs:5` 定义 `const CHINA_OFFSET_SECONDS: i32 = 8 * 60 * 60;`。
更直接的问题——**海外标的的源时间被盖上 `+08:00`**：

```rust
// crates/magic-sina-rs/src/global.rs:105   （外汇报价，USD/JPY 等）
let source_at = format!("{date}T{time}+08:00");
```

成交时段枚举 `MarketSession`（`magic-market-core/src/signals.rs:511-520`）只有
`PreOpen / OpeningAuction / Continuous / LunchBreak / Close / PostClose`——**没有隔夜、
没有盘前盘后、没有跨时段**。海外市场（尤以美股为准）在类型上无法表达其时段。

## 4. 已有的"海外"表示是闭枚举，不能推广

今天的海外表示是**按 provider 的符号表开闭枚举**：

```rust
// crates/magic-market-core/src/global.rs:5-14
/// Global index identities admitted by the verified Sina packet family.
pub enum GlobalIndexCode { DowJones, NasdaqComposite, Sp500, Nikkei225, HangSeng, Ftse100 }
```

文档注释是决定性的：**Core 的契约枚举由"已验证的 Sina 包族"定义**。这意味着"加 KOSPI"
等于**把 Sina 碰巧发布的符号写进 Core 契约**。`FxPair`（`:69-78`）同构。

**同一个"指数"概念在本仓有两种身份表示**：

| | A 股指数 | 海外指数 |
| --- | --- | --- |
| 表示 | `InstrumentId`（`asset_class == Index`） | 闭枚举 `GlobalIndexCode` |
| 请求 | `IndexQuotesRequest { indices: Vec<InstrumentId> }`（`derived_products.rs:27-30`） | `GlobalIndexRequest { indices: Vec<GlobalIndexCode> }`（`global.rs:18-20`） |
| provider | Tencent | Sina |
| 校验 | `validate_six_digit_code` + 拒 `Beijing` | `validate_identities(..., 20)`，去重（`global.rs:152-177`） |

这个分裂本身就是 §3.1/§3.2 的产物：因为 `InstrumentId` 装不下 A 股以外的市场，海外只能另开
一套枚举。

**加指数点位的成本（相对便宜，但有前提）**：Core 加一个枚举变体 + provider 符号映射 + 测试，
**零 proto 变更**（请求体走 `CanonicalPayload` JSON）。代价是 §4 的契约来源问题。

## 5. 来源侧：没有任何已批准或计划中的海外行情来源

即使身份层解决，**海外个股行情目前没有任何来源在册**：

- 仓库自己的海外研究 backlog（`docs/evidence/2026-10-01-news-source-expansion-research.md`）
  第 10–19 条候选**全部是新闻 / 披露 / 宏观**：SEC 披露扩展（`:59`）、Fed/ECB（`:60`）、
  OFAC 制裁（`:61`）、BIS + FederalRegister（`:62`）、欧盟理事会（`:63`）、外交部 + 联合国
  （`:64`）、行业协会（`:65`）、EIA/IEA/OPEC（`:66`）、Reuters/LSEG（`:67`）。
  **没有一条是海外个股行情。**
- 该文档自述**尚未批准合同变化**（`:88`），它是调研提案，不是设计。
- `docs/integrations/unadmitted-provider-routes.md` 的 11 行**全是中国境内 provider**
  （HithinkFinance / EastmoneyMiaoxiang / Jin10 / Imf / Baidu / EmQuant / SecuritiesTimes），
  **没有一行海外**。
- 没有任何文档提出日股或韩股行情路线图。

**准入路径（AGENTS.md 强制）**：任何新增海外行情源都需要
(a) 获批的 Gate A 设计、(b) `docs/integrations/http-transports.tsv` 登记其 HTTP/TLS 依赖、
(c) `docs/integrations/admissions.tsv` 的 BR-009 行（≥2 次有界实网探针 + ≥3 次串行加载）。
本设计**不授予任何数据权利、不登记任何端点**。

## 6. 附带发现：两个硬编码 admitted 的海外路由（对第八个问题的修正）

今天在对外能力服务里被标为"已准入"的海外相关路由，有两条**既无常量也无台账行**：

| operation | 注册点 | 常量 | 台账行 |
| --- | --- | --- | --- |
| `GlobalIndices` | `grpc_production.rs:625-641`，`admitted(..., "Sina", "verified global index identities")` | 无 | 无 |
| `ForeignExchange` | `grpc_production.rs:643-659`，`admitted(..., "Sina", "verified currency-pair identities")` | 无 | 无 |
| `NorthboundDaily` | `grpc_production.rs:2143-2166`，`admitted(..., "Hkex", "official Shanghai- or Shenzhen-connect northbound daily statistics for an exact date")` | 无 | 无 |

核实：

- `magic-sina-rs` 在 `admissions.tsv` 里**一行都没有**（grep `Sina|sina` → 无匹配）。
- `magic-exchange-rs` 只声明**一个**常量 `CFFEX_2026_FUTURES_DELIVERY_ADMITTED`
  （`crates/magic-exchange-rs/src/cffex.rs:30`），`admissions.tsv:37` 也只有 Cffex 一行；
  HKEX 北向的 `northbound_daily_statistics: true`（`crates/magic-exchange-rs/src/hkex.rs:122`）
  是硬编码字面量。
- HKEX 北向有实网证据（`docs/integrations/exchange-official.md:13`，2026-07-22 沪/深股通各
  1 条 + Top10；`:204-208` `live_probe_status=passed`），但**证据 ≠ BR-009 台账行**。

**这修正了第八个问题设计的分组启发式。** 该设计的 §2.2 用"crate 无常量、无台账行"筛选出
7 个 crate，但 `magic-exchange-rs` **不在其中**——它声明了一个路由的常量，于是通过了
`check_admissions.py`，而它的**第二个**路由靠硬编码字面量 admitted。
准确表述应是：**"crate 级"不是正确的粒度，"Provider × operation"才是。**
这与 `docs/integrations/2026-10-14-admission-index-coverage-gate-a-design.md` 的 D1 是同一问题，
本设计不重复拍板，只把这两条路由登记为 D1 的输入。

（附带重申 §2：HKEX 北向是**外资流入 A 股**的统计，不是港股数据；
`magic-exchange-rs/src/hkex.rs:475-524` 把 Top10 证券映射到 `Exchange::Shanghai|Shenzhen`，
`:489` 要求源代码为六位数字。港股个股仍为 0。）

## 7. 拟议修改（待 §8 拍板）

### 7.1 零运行时行为变更（可立即做）

1. **修正 `docs/data-sources-inventory.md` 的失效路径**：第 68–69 行把美股三指数与
   美元/人民币汇率归到 `src/data_gateway/global_market.rs:13-14`——该路径**在本仓不存在**
   （本仓无根 `src/`）。同一缺陷在第七个问题已记录过。须改为真实 crate 路径或标注为下游仓。
2. **在 `data-sources-inventory.md` 显式声明海外覆盖边界**：明确"海外 = 6 指数点位 + 8 汇率，
   **无个股**"，避免"美股三大指数"被读成"支持美股"。
3. **台账补行**（挂第八个问题设计的 D1/D2）：`GlobalIndices` / `ForeignExchange` /
   `NorthboundDaily` 三条路由的准入记录。

以上三条**不涉及任何运行时逻辑**，且即使永远不做海外个股也照样成立。

### 7.2 需要先拍板才能动的

4. **身份层**（D1）、**代码格式**（D2）、**币种**（D3）、**时区/时段**（D4）。
5. **市场与切片范围**（D5）、**来源准入路径**（D6）。

### 7.3 本设计明确不做

- 不选定任何市场、不接触任何海外源、不登记端点。
- 不改 proto、不改 `contract_sha256`、不改任何 provider 的运行时行为。
- 不引入时区库依赖（`chrono-tz` 等）——是否引入是 D4 的内容。

## 8. 待批准人拍板（按编号回答）

- **D1 — 身份表示**：
  (a) 给 `Exchange` 加海外变体（并同步 `SourceExchange`）——最直接，但触发 23 + ~30 + 13 处站点；
  (b) 新增平级类型（如 `MarketVenue` / `Listing`），`InstrumentId` 保持 A 股语义，海外走自己的
  `InstrumentId` 家族——**不动 23 处 match**，但仓库出现两套身份；
  (c) 全部走闭枚举路线（照 `GlobalIndexCode`）——**每加一个市场就改 Core 契约**（§4），
  且个股不可行。
- **D2 — 代码格式**：(a) 按 venue 分支放宽 `validate_six_digit_code`；(b) 引入 canonical ticker
  类型（把"六位数字"从隐含约定变成显式类型，一次性收敛 ~30 处）；
  (c) 不动，海外个股永远不进 `InstrumentId`（与 D1(b) 配套）。
- **D3 — 币种**：(a) 给 `Price` 加币种（触及全仓价格构造点）；
  (b) **在记录层加 `currency: CurrencyCode` 字段**（复用已有类型，不动 `Price`）；
  (c) 只把 `CurrencyCode` 接进 `GlobalIndexQuote` / `FxQuote`（最小切片）。
- **D4 — 时区与时段**：(a) 引入时区库并为每个 venue 建会话表；
  (b) 沿用参数化的 `unix_seconds_to_fixed_offset_rfc3339`（`time.rs:76-103`）按 venue 传偏移，
  不引入依赖；(c) 海外标的的 `source_at` 继续记录**源的时区**并显式声明该语义（最小改动，
  但须写清"这不是标的本地时间"）。
  **与本仓第二个 Gate A 设计（交易日历）耦合**——海外市场需要各自的交易日历。
- **D5 — 市场与切片**：先做哪个市场、先做指数还是先做个股。本设计的排序建议：
  **美股 > 港股 > 日股 > 韩股**（依据：美股已有 SEC 元数据基础与最多用例；日股仅 1 个指数；
  韩股连指数都没有且无任何来源线索）。
- **D6 — 来源准入路径**：是否先做"只加指数点位、不碰身份层"的最小切片（§12.3），
  还是等 D1 定案后一次性做个股。**最小切片会把 provider 符号表写进 Core（§4）。**

## 9. 被否决的替代方案

- **直接加 `Exchange::HongKong` / `Tokyo` 等变体了事**：真正的否决理由不是 23 处编译错误
  （那些会报错，好办），而是 **~30 处静默站点**——它们会编译通过，然后静默地把海外标的
  塞进 A 股专用逻辑（`HashSet::from([沪,深,京])` 静默排除；`== Beijing` 特判静默放行）。
  在补完这 30 处之前，加变体是**把 bug 静默化**，违反 Gate B 的"显式失败"原则。
- **把 `GlobalIndexCode` 的闭枚举模式推广到个股**：等于把 provider 的符号表写进 Core 契约
  （§4 的文档注释已明示这一点）。且个股无法枚举。
- **靠 `CanonicalPayload` JSON 绕过 proto 改动**：技术上成立（新增变体不改 proto），
  但**不解决任何身份问题**——`InstrumentId` 是请求体里的 serde 类型，未知 `exchange` 变体
  直接 decode error。这一点其实是**正面**的：当前边界**显式失败**（Gate B），
  改身份层时必须保住这个性质。
- **在本设计里重新跑实网探针**：本设计不执行任何探针。Sina 是否支持 KOSPI 符号属未验证项（§11）。

## 10. 影响面与风险

- **本设计本身**：零 proto、零 `contract_sha256`、零运行时行为变更、零新依赖。
  §7.1 的三条只改 Markdown 与台账。
- **若 D1 选 (a)/(b)**：`Exchange` 是 JSON 请求体里的**对外身份类型**。任何改动都影响下游
  已冻结的身份契约，需要 Gate A 批准 + 下游协调。**这是本设计把 D1 单列为主决策点的原因。**
- **主要风险是那 ~30 处静默站点**：编译器不会报错，只能靠穷举审查与新增的穷尽 `match`
  把它们从"静默"变成"编译期强制"。
- **对 Gate C 的影响**：本仓无 PowerShell 7，`tools/docs/check_links.ps1` 不可运行，
  文档链接检查需人工完成。`docs/integrations/` 内一律用反引号路径而非相对链接（house style）。
- **bundle 影响**：`tools/docs/build_client_bundle.ps1:61-68` 是**白名单**，本设计不会自动进入
  客户端 bundle。
- **负价值边界**：本设计**不授予任何数据权利、不恢复任何授权、不新增任何 capability**。
  它不使任何海外数据变得可用。

## 11. 未验证项（不写成结论）

- **未运行任何实网探针。** 未验证 Sina 是否支持韩股 / 台股等其他 `int_*` 符号——若支持，
  §12.3 的最小切片才成立；若不支持，加枚举变体也无源可用。
- 未验证 ~30 处静默站点的**精确计数**：该数字是穷举审查的**判断性上界**，部分站点是校验守卫，
  其"新变体行为"取决于下游代码形状检查。
- 未审计 `tests/` 内是否存在额外的穷尽 `match`（已核实 3 处 example 目标）。
- **未确认下游是否有真实用例**，以及用例指向哪个市场、指数还是个股。D5 的排序建议基于
  仓内证据（来源成熟度），不是基于已知需求。
- 未确认 `docs/data-sources-inventory.md` 中的 `src/data_gateway/**` 路径是否描述一个**下游仓**
  （该仓不在本仓内）；本设计只主张"该路径在本仓不存在"。
- 未验证本仓 `docs/evidence/2026-10-07-upstream-live-probes-230223/` 海外面是否有失败：
  该批次的 `magic-sina-rs` 探针为 **failed**（`Error: Decode("option month JSON: ...")`，
  ETF 期权族），**不涉及** global/FX 族；因此不能据此推断海外路由的健康状况。

## 12. 价值评估与拆分建议（2026-10-14 补充）

### 12.1 零运行时风险的修正（确定收益）

§7.1 的三条：修正 `data-sources-inventory.md` 的失效路径、显式声明海外覆盖边界、台账补三条
路由的准入记录。

修的是"对外文档把'美股三大指数'读成'支持美股'"这类**活陷阱**——按当前文档，读者会以为
海外个股已经可用。这三条**即使永远不做海外个股也照样成立**。

### 12.2 真正的前置条件

**D1 身份层。** 不做 D1，海外个股无法落地，任何"接一个美股源"的工作都会在第一步卡住。
这不是本设计引入的约束，是 §3.1 已存在的事实。

### 12.3 最小可行切片及其代价

**只加指数点位**（如 `GlobalIndexCode::Kospi`）：Core 加变体 + Sina 符号映射 + 测试，
**零 proto、不动 23 处 match**。

但代价有二，必须写清：
1. **它把 provider 符号表写进 Core 契约**（§4）——每加一个市场都要改 Core；
2. **它不解决个股**，且会让"海外已支持"的错觉更强。

因此本设计**不把最小切片列为默认路径**，而是列为 D6 的一个选项。

### 12.4 明确不做的

- 不选市场、不接源、不登记端点、不跑探针。
- 不改 proto / `contract_sha256` / 运行时注册逻辑。
- 不改 `Exchange` 或 `SourceExchange`（那是 D1 拍板后的 Gate B 工作）。
- 不修第八个问题设计里的其他条目——本设计只把两条硬编码路由作为 D1 的输入登记（§6）。
