# TDX 板块族（目录 / 精确成分 / 反向成分）准入与证据

本文件是 `magic-tdx-rs` 板块三族的 BR-009 准入记录，同时承载
`BOARD_DIRECTORY_ADMITTED` / `BOARD_CONSTITUENTS_ADMITTED` /
`BOARD_MEMBERSHIPS_ADMITTED` 三个常量（声明制，无运行时门；三条路由本就注册上线，
本次只补齐台账行——见 `2026-10-14-tdx-board-zs-gate-a-design.md` §6.4 拍板）。

## 1. 生产接线与身份

| operation | 注册点（`crates/magic-market-composition/src/grpc_production.rs`） | provider |
| --- | --- | --- |
| `BoardDirectory` | `register_tdx_boards_client`（`~2650`） | `Tdx` |
| `BoardConstituents` | 同上（`~2671`） | `Tdx` |
| `BoardMemberships` | 同上（`~2688`） | `Tdx` |

- 数据面：TDX 公开协议板块文件 `block_fg.dat`（行业）/ `block_gn.dat`（概念），
  经 `TdxBlockClient` 读取；`TdxBoardProvider` 规范化（`docs` 规则 BR-014）。
- 板块身份：`tdx:industry:<名称>` / `tdx:concept:<名称>`（BR-026 现行生产身份）。
- `source_at = None`：板块包本身不含时间戳，不臆造；batch id 形如
  `tdx-block-files:<family>:<observed-time>`（无文件哈希）。
- 名称边界：板块文件只有证券代码、没有证券名称；需要名称的消费方按 BR-014
  经 `magic_market_router::join_board_membership_names` 与独立元数据批次拼接。
- 未准入面：`block_zs.dat` 维持"非准入面"（Index 显式拒绝、Region 保持
  `Unsupported`，不冒充指数/地域板块）。`BlockService` 三文件实现保持现状
  （仅服务 `ConceptHits`），不迁移、不复活 router 链。
- 服务器选择（2026-10-15 晚修正）：不再钉死单台主机——与 hq 路径共用
  `PRIMARY_SERVERS` 顺序故障转移，仅连接级故障（建连/超时/握手/断开）切换
  下一台；协议与数据错误原样返回。触发：固定主机 `180.153.18.170` 当晚出现
  连接超时与空 F10 节，同时 `117.34.114.14` 直连探针正常（目录 269 / 成分
  400 / 反向 24）。

## 2. 有界实网证据（2026-10-15 批次）

证据目录 `docs/evidence/2026-10-15-upstream-live-probes-214528/`（含 `summary.json`、
逐次 stdout/stderr 与二进制 SHA-256）。样本板块 `人工智能`（category=concept）、
服务器 `117.34.114.14`：

| 路由 | 实测（live ×2） | 串行加载（并发 1，每轮 ≤3 请求） |
| --- | --- | --- |
| 目录 | `directory_records=269`，目标板块存在；`board_live_probe_status=passed` | 3 轮 × 3 请求全部成功 |
| 成分 | `constituent_records=400` | 3 轮 × 3 请求全部成功 |
| 反向成分 | `000016` → `reverse_records=24` | 3 轮 × 3 请求全部成功（每轮 3/3，约 48–50s） |

两次 live 运行与九次加载运行全部通过；台账行以"2 live + 3 serial（每路由）"
登记。这是一次有界复探批次，不构成持续健康监测。

## 3. 边界与规则引用

- BR-014：板块目录/成分只来自经验证的 `block_fg.dat` / `block_gn.dat` 记录；
  重复板块/成员对报错；反向成分不返回未请求的证券；缺名/多身份/覆盖不完整报错。
- BR-026：规则文本已按两文件生产实现对齐（2026-10-15，含 `tdx:industry:` /
  `tdx:concept:` 身份与北京前缀边界）。
- 本文件不授予任何数据权利、不执行探针、不改变任何运行时行为。
