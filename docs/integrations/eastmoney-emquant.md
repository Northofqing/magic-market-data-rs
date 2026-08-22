# 东方财富 EMQuant 只读接入

## 已探测到的本地 SDK

历史 macOS 开发机存在官方 EMQuant C++ SDK：

```text
/Users/zhangzhen/Downloads/EMQuantAPI_CPP_Mac/
├── x64/bin/libEMQuantAPIx64.dylib
├── x64/EmQuantAPI/EmQuantAPI.h
└── x64/EmQuantAPITestExe/main.cpp
```

2026-08-16 Windows 开发机安装 Choice 9.14.0.1，并从 Choice 量化接口官方下载
C++ SDK 2.7.5.0 到 Git 忽略的 `target/emquant-sdk/`。下载包 SHA-256 为
`9f3f32160b61be6cdd4dd1be260c984072364c6df88724724d0697ffeecdef89`，包含
`EmQuantAPI.h`、`EmQuantAPI_x64.dll`、`ServerList.json.e` 和官方激活工具。

SDK 版权声明要求授权使用；构建脚本只把本机动态库复制到 Git 忽略的运行目录，
不会提交厂商文件、账号或令牌。

## 映射范围

| 项目契约 | EMQuant API | 状态 |
| --- | --- | --- |
| 实时 Quote | `csqsnapshot` | Rust 适配完成；2026-08-21 Windows 实测仍为字段权限 `10001012`，未准入 |
| 完成日线 | `csd` | 2026-08-21 Windows 两次 focused probe、四次串行请求通过；精确范围生产准入 |
| 周/月/年 K 线 | `csd` | 代码可用但未独立完成当前授权实测，未准入 |
| 1/5/15/30/60 分钟线 | `chmc` + Rust 聚合 | Rust 适配完成；2026-08-21 Windows 实测仍为服务权限 `10001012`，未准入 |
| 逐笔 | `chq` | 字段与分页尚未验证，显式 `Unsupported` |
| 盘口/Level-2 | `csqsnapshot` 五档指标 | Rust 适配完成；2026-08-21 Windows 实测仍为字段权限 `10001012`，未准入 |
| 日级资金流 | `css` 大/中/小单流入流出指标 | 2026-08-21 返回证券行但全部分档值为空，未准入 |
| 开盘集合竞价 | 未找到完整可验证字段集 | 显式 `Unsupported` |
| 证券元数据 | 未完成字段与源时间验证 | 显式 `Unsupported` |

## 安全边界

- 只接入行情、历史和研究数据；不调用组合、转账、下单、持仓或资金接口。
- `setproxy` 仅用于用户明确配置的合法代理，不做代理抓包或 TLS 绕过。
- 每次返回都必须填充 `provider=Eastmoney`、`observed_at`、批次 ID；只有源明确提供时才填 `source_at`。
- 未授权、权限不足、字段缺失或源时间不可证明时返回结构化错误/`Unsupported`，不能填 0。

## 可选部署覆盖

```text
MAGIC_EMQUANT_LIB=/absolute/path/to/libEMQuantAPIx64.dylib
MAGIC_EMQUANT_SERVER_LIST=/absolute/path/to/sdk/x64/bin
MAGIC_EMQUANT_USERNAME=旧版授权账号  # 可选，必须与密码同时设置
MAGIC_EMQUANT_PASSWORD=旧版授权密码  # 可选，必须与账号同时设置
```

Windows 覆盖路径使用 `EmQuantAPI_x64.dll`；默认同级 runtime 布局无需设置覆盖变量。

账号、密码和激活令牌不进入仓库、日志或测试 fixture。EMQuant 2.0 及以后按照官方
头文件说明，默认使用与 `ServerList.json.e` 同目录的 `userInfo` 激活令牌自动登录，
无需传账号密码。正式接入前必须先用官方激活工具/示例确认登录、Quote 权限和
Level-2 权限，再启用对应 capability。

本地 SDK 文件布局和示例语法可用以下只读检查验证：

```text
bash tools/emquant/check_sdk.sh /path/to/EMQuantAPI_CPP_Mac
```

只读快照桥接程序可独立构建；新版 SDK 使用官方激活令牌，旧版 SDK 的可选账号和
密码只从进程环境读取。完成构建后，下列三个路径变量都不是必需项：

```text
tools/emquant/build_snapshot_bridge.sh /path/to/EMQuantAPI_CPP_Mac
target/emquant/emquant-snapshot 600396.SH PRECLOSE,OPEN,HIGH,LOW,NOW,AMOUNT
```

Windows x64：

```text
tools\emquant\build_snapshot_bridge_windows.cmd C:\path\to\EMQuantAPI_CPP
target\emquant\emquant-snapshot.exe 600396.SH PRECLOSE,OPEN,HIGH,LOW,NOW,AMOUNT
```

程序只调用 `csqsnapshot`、`csd`、`chmc` 和 `css` 的只读查询，向标准输出写
JSON；错误信息不包含凭据。

Rust 适配层和实盘探针：

```text
cargo run -p magic-emquant-rs --example live_probe --release
cargo run -p magic-emquant-rs --example daily_bars_probe --release --locked --offline
```

构建脚本无论从哪个目录调用，都会把桥接程序放到仓库固定路径
`target/emquant/emquant-snapshot[.exe]`，把加密服务器列表和 SDK 动态库复制到 Git 忽略的
`target/emquant/runtime/`，复制激活器的 `image/` 资源，并在那里建立指向存在时的
`userInfo` 激活文件的权限受限本地副本。
在 macOS 上，脚本会清除动态库复制件的下载隔离属性，并只对复制件执行本机临时
签名，以免被系统的动态库验证策略拒绝；Downloads 中的厂商原文件不会修改。
Rust 适配层和桥接器
会自动发现这些项目内文件，不再要求每次设置 `MAGIC_EMQUANT_BRIDGE`、
`MAGIC_EMQUANT_LIB` 或 `MAGIC_EMQUANT_SERVER_LIST`。只有部署时需要覆盖项目内默认
路径，才设置对应变量；SDK 与激活文件不会提交到仓库。

若探针返回 `10001014 (EQERR_NEED_ACTIVATE)`，执行脚本已经准备并本机临时签名的
`target/emquant/runtime/loginactivator_mac`（macOS）或
`target/emquant/runtime/LoginActivator.exe`（Windows），在官方界面完成 API 激活。该程序与
`ServerList.json.e` 位于同一目录，成功后会在这个 Git 忽略目录生成 `userInfo`。
东方财富桌面客户端登录与 EMQuant API 激活是两套独立会话，前者不会替代后者。
macOS 激活器依赖 GTK 3，界面使用账号绑定手机号与短信验证码，不提供用户名密码
输入框。`userInfo` 应设为仅当前用户可读写，且绝不能提交、打印或复制进发布包。

2026-08-16 Windows 设备已成功生成 `userInfo`，证明设备激活完成；随后 bridge 和
完整 Rust live probe 均收到 `10001004 (EQERR_ACCESS_EXPIRE)`。Quote、五档、日级
资金流、日线和 5 分钟线全部被同一账号级服务端权限门阻断，没有任何查询记录产生。
该结果不是字段名、Windows loader 或解析器失败。必须由 Choice 量化接口后台或客户
经理恢复 EMQuant API 权限后重跑；Choice 桌面终端登录不能绕过此门。

2026-08-21 恢复 15 天 API 权限后，focused probe 对 `600396.SH`、`000001.SZ` 完成
两轮、共四次串行 `csd` 请求，显式区间 `2026-08-18..2026-08-20` 均返回三条完整、
严格递增的未复权日线，OHLC、成交量、成交额、日期和批次证据全部通过。因此仅将
沪深股票 `interval=Day`、显式起止日期、最多 800 条完成日线提升为生产
`HistoricalBars(provider=EmQuant)`。日线记录中的真实来源身份仍为 `Eastmoney`；
`EmQuant` 是 gRPC 的 SDK 接入选择名，不会重标来源 evidence。

同一次授权窗口内，Quote、五档和 `chmc` 仍返回
`10001012 (EQERR_ACCESS_INSUFFICIENCE)`，`css` 虽返回证券行但资金流字段全为空，
因此均保持未准入。权限到期、激活失效或运行时缺失时，生产日线返回无 records 的
类型化 unavailable；不得把权限失败解释成 verified-empty，也不得填 0、旧日数据或
其他 Provider 数据。显式区间包含尚未完成且字段为空的当日时，同样整批失败。

本机在 2026-07-23 再次完成短信激活并开通 Choice 权限后，关闭激活器的干净 SDK
进程已成功登录。真实 `css` 取得华电辽能、平安银行的完整日级资金流，真实 `csd`
取得华电辽能最近五根日线。SDK 返回的日线日期采用 `YYYY/M/D`；Rust 适配层已经
增加补零标准化和回归测试，输出统一为 `YYYY-MM-DD`。

历史账号的 Quote、五档和 `chmc` 分钟线查询同样返回过
`10001012 (EQERR_ACCESS_INSUFFICIENCE)`。本机官方 `EmQuantAPI.h` 将其定义为权限
不足：账号和 API 登录已经有效，但当前产品没有覆盖这些数据服务或字段集。需要在
Choice/QuantAPI 后台追加并确认 Quote、Level-2 和分钟历史的实际权限，再分别重跑
probe；不能用日线成功推断其他数据族也有权限。bridge 已对无权限、权限不足、权限
过期、Level-2 无权限、登录数上限、设备不一致和令牌过期输出可操作诊断。

默认查询 `600396.SH,000001.SZ`（华电辽能、平安银行），也可通过
`MAGIC_EMQUANT_CODES` 修改。输出包含
实时价、成交量、成交额、五档买卖价量、可见买卖总深度及逐记录证据，以及第一只证券最近五根不复权日线和
5 分钟线的 OHLCV、成交额、源时间、采集时间和批次 ID。日/周/月/年 K 线使用官方 `csd`，
明确传入 `Period=1..4,AdjustFlag=1,Order=1`；空结果、代码错配、重复/逆序日期和
OHLC 不一致均报错。若登录或权限不足，命令明确失败，不会回退到测试数据。Rust
适配层默认在 30 秒后终止无响应的 SDK 子进程，可通过正整数
`MAGIC_EMQUANT_TIMEOUT_SECS` 调整。

本机 SDK 头文件和随包官方示例声明 `chmc` 分钟 K 线接口；Rust 适配层只用它拉取
原始分钟 OHLCV，再本地严格聚合为 5/15/30/60 分钟。当前官网 2.7.3 公共手册未列
该接口，因此代码能力已完成，但当前 `chmc` 查询返回
`10001012/EQERR_ACCESS_INSUFFICIENCE`，不能标为实盘已验收。

日级资金流通过 `css` 请求 `SUPER/BIG/MID/SMALL` 各档流入和流出金额，逐档计算
净额，主力净额定义为超大单净额加大单净额。任何一档缺失都保留为
`Unavailable` 并把批次标为不完整；该能力不冒充 5 秒实时资金流。字段依据来自
Choice 官方升级指标记录。2026-07-23 的真实 probe 已取得两只默认证券的完整记录。

官方资料只能证明收盘集合竞价成交量/额，无法同时证明开盘竞价所需的撮合价、
匹配量、未匹配买量和未匹配卖量。因此 `Auctions` 已实现为明确的 `Unsupported`
错误；在获得完整授权字段定义前不拼接 Quote 或 0 值伪造。

## 本地客户端探测结论

2026-07-22 在 macOS 东方财富客户端已登录并运行期间做了只读进程检查：客户端建立
了到远端 1860/1862 行情端口的出站连接，但没有开放归属于该进程的本地 TCP 监听
端口。客户端沙箱内确有证券字典、财务/除权、板块数据库以及浏览过证券的
`DetailData` 二进制缓存；后者会随页面访问更新，但覆盖范围只限当前缓存证券，且
未发现官方格式或稳定 IPC 合同，不能据此承诺全市场实时 Quote、盘口或集合竞价。
因此项目不代理、解密或重放客户端私有连接，也不读取账号、交易日志或登录令牌；
这些缓存至多作为未来经格式授权后的补充源，可审计的实时接入路径仍是官方 EMQuant
SDK。
