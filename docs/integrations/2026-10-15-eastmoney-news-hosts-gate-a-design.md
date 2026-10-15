# Gate A 设计：东财 GlobalNews 文章宿主白名单漂移（2026-10-15）

状态：已批准（D1a / D2a / D3a，2026-10-15）并已实施（§9）；D1 于同日修订为结构制（§10）
问题 op：`GlobalNews`（provider=Eastmoney 一条腿；无新增 op、无 proto、无合同字段变更）

## 1. 问题

Mac 侧 global_news 调用自 2026-10-14 13:51（本地）起连续约 19 小时被服务端
FailedPrecondition 拒绝，实时新闻零摄入（朱雀二号火箭 2026-10-14 19:40 发射案例只是
其中一条）。根因：东财快讯 feed 持续混入 `money.eastmoney.com` /
`insurance.eastmoney.com` 文章链接，两者不在 provider 的 10 宿主白名单内，任一出现即
整批 fail-closed。

## 2. 已核实证据（全部实测）

### 2.1 失败签名与窗口

- 服务端日志：`operation=global_news stage=source_precondition_failed` 自
  2026-10-14T05:51:50Z 起约 30 秒一条，持续至 2026-10-15T03:38Z 仍在出现。
- gRPC 实测（2026-10-15 11:50 起，limit=5/20 均现）：`status=9`，
  message=`Eastmoney news article host "money.eastmoney.com" is not an admitted
  global-news host`（偶发 `insurance.eastmoney.com`）。
- 对照：同日 11:31 `Eastmoney limit=5` 曾成功一次（当时窗口前 5 条恰好全在白名单
  宿主）——证明是内容漂移型失败，非传输/配置问题。
- 其它 8 条新闻腿（默认 WallstreetCn、Cls、Cailianpress、ThePaper、XinhuaFinance、
  Yicai、Yonhap、Jin10）同期逐源实测全部成功。

### 2.2 白名单位置与性质

- `crates/magic-eastmoney-rs/src/news.rs:292-302`：finance / global / biz / stock /
  fund / futures / bond / bank / forex / hk 十宿主；`:305` 未列入即
  `EastmoneyError::Protocol` 整批拒绝。
- `item_id` 由 `/a/` 路径派生（`:310-311`），与宿主无关——扩宿主不影响 item 身份
  稳定性。
- `docs/integrations/eastmoney-web.md` 与 `admissions.tsv` 均无该白名单的声明
  （见 §2.5 台账缺口）。

### 2.3 线上 feed 组成（2026-10-15 12:01 抓取 `https://roll.eastmoney.com/finance.html`，48,440 字节）

40 条文章链接：finance×35、money×1、insurance×1、futures×1、bond×1、bank×1。
money./insurance. 约占 5%——滚动窗口下 20 条窗口含 ≥1 个未准入宿主的概率持续居高，
解释了 19 小时近似连续失败。

### 2.4 一方性证明（curl 回执）

- `money.eastmoney.com/a/202610153896649090.html` → 302 → `https://money…` → 302 →
  `https://finance.eastmoney.com/a/202610153896649090.html` → 200
- `insurance.eastmoney.com/a/202610153896646423.html` → 同链归并 → finance.eastmoney.com 200
- `finance.eastmoney.com/a/…` → 302 → https → 200（一跳）
- 页面标记：money. 文章页 `<title>…_ 东方财富网</title>`，eastmoney 标记 147 处。
- 结论：money./insurance. 文章经东财自家重定向链归并到已准入宿主
  finance.eastmoney.com，是同一 CMS 文章的一方别名宿主，不是外站链接。

### 2.5 台账缺口（附带发现）

`admissions.tsv` 无 `magic-eastmoney-rs` 的 `GLOBAL_NEWS_ADMITTED` 行（`:47-52` 只有
资金流/广度/榜单；对照 SecuritiesTimes/WallstreetCn/Xinhua/Yicai 均有该行）。
README:47-48 的权威声明对东财新闻腿不成立——与 admission-index-coverage 设计文档
同一族缺口。

## 3. 拟议修改（推荐组合，待 §7 拍板）

假定 D1=(a)、D2=(a)、D3=(a)：

1. `news.rs` 白名单 +`money.eastmoney.com` +`insurance.eastmoney.com`（注释注明
   2026-10-15 重定向归并证据与本文档路径）。
2. 测试：既有宿主校验测试补 money./insurance. 正例 + 拒例（如 `guba.eastmoney.com`
   仍拒）。
3. `eastmoney-web.md`：补"全球新闻文章宿主准入清单"节——清单、归并证据、漂移条款
   （feed 出现未列入宿主时整批拒绝是设计行为，但台账需注明复核节奏）。
4. `admissions.tsv`：补 `magic-eastmoney-rs  Eastmoney  GLOBAL_NEWS_ADMITTED` 行
   （evidence=`docs/integrations/eastmoney-web.md`）。
5. 部署：与 38edeee（events 序列修复）同批一次构建、一次 Mac 重钉。

## 4. 被否决的替代方案

- **归一化改写 money./insurance. → finance.**：`item_id` 本就与宿主无关，改写无身份
  收益，反把 feed 实际给出的链接改写成另一个值，provenance 真实性倒退。否决。
- **动态重定向验证**（逐文章跟随 302 证明归并）：每文章多一次外呼，引入超时/反爬/
  链变化等新失败面，把准入判断从静态变成运行时依赖。否决。
- **全量 `*.eastmoney.com` 放开**：把股吧/财富号等 UGC 宿主一并纳入新闻身份边界，
  超出本事件证据。否决。
- **跳过坏条目宽松化**：静默丢弃未准入宿主条目使批次不再完整且掩盖漂移，违反
  fail-closed。否决（保留整批拒绝，配套 §3.3 台账复核节奏）。

## 5. 影响面与风险

- 爆炸半径：`news.rs` 一处 `matches!` + 测试 + 两文档；proto/`contract_sha256` 零变化。
- 对外行为：`GlobalNews{Eastmoney}` 从持续 FailedPrecondition 恢复为返回数据；
  item 身份 schema 不变。
- 部署连带：重建 → `binary_sha256` 变 → Mac 重钉（与 38edeee 同批，一次重钉）。
- 残留风险：feed 未来漂到其它别名宿主（如 caifuhao）会再次 fail-closed——接受
  （这是设计），配套是 §3.3 的台账复核节奏与 §6 的可观测性缺口。

## 6. 附带发现（不在本设计范围）

服务端日志只落 stage 不落 error message——19 小时故障里决定性报文
（"host not admitted"）只有 Mac 侧可见。建议 `app.rs` 的 service-failure 日志带出
message（小改，另案）。

## 7. 待批准人拍板（按编号回答）

- **D1 — 处置方式**：(a) 白名单 +`money.` +`insurance.`（荐：`item_id` 与宿主无关、
  provenance 如实、§2.4 证据）；或 (b) 归一化改写（§4 已否决）。若认为保险频道不应
  入"全球财经新闻"边界，可只 +`money.`。
- **D2 — 路径**：(a) 单独快速立案（荐：生产在烧）；或 (b) 并入
  2026-10-15-news-data-coverage Gate A 作第四条治理项。
- **D3 — 台账**：(a) `eastmoney-web.md` 补清单 + `admissions.tsv` 补行（荐）；或
  (b) 只改代码与测试。

## 8. 未验证项（不写成结论）

- insurance. 文章内容逐篇语义未审阅；页面结构为一方东财新闻（§2.4），若拍板排除
  保险频道见 D1。
- Mac 聚合层把单源失败升级为整批失败的具体规则（`service.rs` 完成阈值）未逐行追完；
  本设计只负责服务端东财腿恢复。若恢复后 Mac 仍缺新闻，下一案查 Mac 聚合规则。

## 9. 实施记录（2026-10-15，D1a / D2a / D3a）

- `crates/magic-eastmoney-rs/src/news.rs`：白名单 +`money.eastmoney.com`
  +`insurance.eastmoney.com`，附证据注释。
- `crates/magic-eastmoney-rs/src/lib.rs`：新增 `GLOBAL_NEWS_ADMITTED` 常量，
  `content_capabilities().global_news` 改引用常量（BR-009 台账钩子）。
- `crates/magic-eastmoney-rs/tests/internal/global_news_regression_tests.rs`：
  官方宿主正例表补 money./insurance.；新增 `guba.eastmoney.com` 拒绝断言。
- `docs/integrations/eastmoney-web.md`：宿主边界块与元数据条款补全清单、归并证据、
  漂移条款。
- `docs/integrations/admissions.tsv`：新增
  `magic-eastmoney-rs  Eastmoney  GLOBAL_NEWS_ADMITTED  true` 行。
- 门禁：`cargo test -p magic-eastmoney-rs` 228 项全绿；`cargo fmt --check` 干净；
  `clippy --all-targets -- -D warnings` 零警告；`tools/compliance/check.sh`（含
  BR-009）通过。
- 实网探针：`MAGIC_EASTMONEY_LIVE_OPERATION=global-news`
  （`MAGIC_EASTMONEY_POOL_DATE=2026-10-15`）串行 3 次执行，3/3
  `live_probe_status=admitted`、`failures=0`、records=5；三次窗口实测宿主
  finance/futures/stock（money./insurance. 属窗口相关，其放行由确定性单测覆盖；
  feed 抓取实证见 §2.3/2.4）。
- 部署：与 38edeee（events 序列修复）同批构建；`binary_sha256` 变化 → Mac 侧
  一次重钉。
- 残留：运行中的 3faa9d8 服务端仍为旧白名单，部署前东财腿按窗口概率继续失败
  （§1）；部署后缓解。

## 10. 修订二（2026-10-15 当日，D1a'）：名单制改为结构制

§9 上线后，滚动窗当日又漂出 `gold.eastmoney.com`（`/a/<id>` 302 归并
`finance.eastmoney.com`，与 money./insurance. 同模式）——名单制被持续漂移抛离。

- **新规则**：宿主以 `.eastmoney.com` 结尾（子域形态）且**不**属于 UGC 集
  `{guba.eastmoney.com, caifuhao.eastmoney.com}`；文章路径仍须严格
  `/a/<纯数字>.html`（既有校验）。规则实现于 `news.rs` 的宿主检查，测试改写为
  13 宿主正例表 + 4 负例（guba/caifuhao/裸域/伪后缀）。
- **新证据（curl 回执）**：gold. `/a/<id>` → 302 finance. 200；guba `/a/` → 404；
  caifuhao `/a/` → 302 `/404_channel.html`。
- **语义**：`/a/` CMS 文章命名空间即身份边界；只有一方频道宿主服务它，UGC 宿主
  实测不服务，故"后缀 + 排除 UGC"穷尽已知边界。若未来 UGC 宿主开始服务 `/a/`，
  需重新审计并更新本节。
- §4 对"全量 `*.eastmoney.com` 放开"的否决基于当时无归并/404 证据；本修订以回执
  为据把边界精确到"一方频道宿主 × `/a/` 命名空间"，非无差别放开。

