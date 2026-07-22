# 东方财富 EMQuant 只读接入

## 已探测到的本地 SDK

本机存在官方 EMQuant C++ SDK：

```text
/Users/zhangzhen/Downloads/EMQuantAPI_CPP_Mac/
├── x64/bin/libEMQuantAPIx64.dylib
├── x64/EmQuantAPI/EmQuantAPI.h
└── x64/EmQuantAPITestExe/main.cpp
```

SDK 版权声明要求授权使用；项目不复制动态库、不提交账号或令牌，运行时通过
用户配置的绝对路径加载。

## 映射范围

| 项目契约 | EMQuant API | 状态 |
| --- | --- | --- |
| 实时 Quote | `csqsnapshot` | Rust 适配完成，待授权登录验证 |
| 日/周/月/年 K 线 | `csd` | Rust 适配完成，待授权登录验证 |
| 1/5/15/30/60 分钟线 | `chmc` + Rust 聚合 | Rust 适配完成，待授权登录验证 |
| 逐笔 | `chq` | 取决于权限 |
| 盘口/Level-2 | `csqsnapshot` 五档指标 | Rust 适配完成，待 Level-2 权限验证 |
| 日级资金流 | `css` 大/中/小单流入流出指标 | Rust 适配完成，待授权登录验证 |
| 开盘集合竞价 | 未找到完整可验证字段集 | 显式 `Unsupported` |

## 安全边界

- 只接入行情、历史和研究数据；不调用组合、转账、下单、持仓或资金接口。
- `setproxy` 仅用于用户明确配置的合法代理，不做代理抓包或 TLS 绕过。
- 每次返回都必须填充 `provider=Eastmoney`、`observed_at`、批次 ID；只有源明确提供时才填 `source_at`。
- 未授权、权限不足、字段缺失或源时间不可证明时返回结构化错误/`Unsupported`，不能填 0。

## 配置约定

```text
MAGIC_EMQUANT_LIB=/absolute/path/to/libEMQuantAPIx64.dylib
MAGIC_EMQUANT_SERVER_LIST=/absolute/path/to/sdk/x64/bin
MAGIC_EMQUANT_USERNAME=旧版授权账号  # 可选，必须与密码同时设置
MAGIC_EMQUANT_PASSWORD=旧版授权密码  # 可选，必须与账号同时设置
```

账号、密码和激活令牌不进入仓库、日志或测试 fixture。EMQuant 2.0 及以后按照官方
头文件说明，默认使用与 `ServerList.json.e` 同目录的 `userInfo` 激活令牌自动登录，
无需传账号密码。正式接入前必须先用官方激活工具/示例确认登录、Quote 权限和
Level-2 权限，再启用对应 capability。

本地 SDK 文件布局和示例语法可用以下只读检查验证：

```text
bash tools/emquant/check_sdk.sh /path/to/EMQuantAPI_CPP_Mac
```

只读快照桥接程序可独立构建；新版 SDK 使用官方激活令牌，旧版 SDK 的可选账号和
密码只从进程环境读取：

```text
tools/emquant/build_snapshot_bridge.sh /path/to/EMQuantAPI_CPP_Mac
MAGIC_EMQUANT_LIB=/path/to/libEMQuantAPIx64.dylib \
MAGIC_EMQUANT_SERVER_LIST=/path/to/sdk/x64/bin \
target/emquant/emquant-snapshot 600519.SH PRECLOSE,OPEN,HIGH,LOW,NOW,AMOUNT
```

程序只解析 `csqsnapshot`，向标准输出写 JSON；错误信息不包含凭据。

Rust 适配层和实盘探针：

```text
MAGIC_EMQUANT_BRIDGE=target/emquant/emquant-snapshot \
MAGIC_EMQUANT_LIB=/path/to/libEMQuantAPIx64.dylib \
MAGIC_EMQUANT_SERVER_LIST=/path/to/sdk/x64/bin \
cargo run -p magic-emquant-rs --example live_probe --release
```

默认查询 `600519.SH,000001.SZ`，也可通过 `MAGIC_EMQUANT_CODES` 修改。输出包含
实时价、成交量、成交额、五档买卖价量、可见买卖总深度及逐记录证据，以及第一只证券最近五根不复权日线和
5 分钟线的 OHLCV、成交额、源时间、采集时间和批次 ID。日/周/月/年 K 线使用官方 `csd`，
明确传入 `Period=1..4,AdjustFlag=1,Order=1`；空结果、代码错配、重复/逆序日期和
OHLC 不一致均报错。若登录或权限不足，命令明确失败，不会回退到测试数据。Rust
适配层默认在 30 秒后终止无响应的 SDK 子进程，可通过正整数
`MAGIC_EMQUANT_TIMEOUT_SECS` 调整。

本机 SDK 头文件和随包官方示例声明 `chmc` 分钟 K 线接口；Rust 适配层只用它拉取
原始分钟 OHLCV，再本地严格聚合为 5/15/30/60 分钟。当前官网 2.7.3 公共手册未列
该接口，因此代码能力已完成，但在获得激活令牌并验证 `chmc` 权限前不把它标为
实盘已验收。

日级资金流通过 `css` 请求 `SUPER/BIG/MID/SMALL` 各档流入和流出金额，逐档计算
净额，主力净额定义为超大单净额加大单净额。任何一档缺失都保留为
`Unavailable` 并把批次标为不完整；该能力不冒充 5 秒实时资金流。字段依据来自
Choice 官方升级指标记录。

官方资料只能证明收盘集合竞价成交量/额，无法同时证明开盘竞价所需的撮合价、
匹配量、未匹配买量和未匹配卖量。因此 `Auctions` 已实现为明确的 `Unsupported`
错误；在获得完整授权字段定义前不拼接 Quote 或 0 值伪造。

## 本地客户端探测结论

2026-07-22 在 macOS 东方财富客户端运行期间做了只读进程检查：客户端建立了到
远端行情端口的出站连接，但没有开放归属于该进程的本地 TCP 监听端口。其打开的
本地数据库属于界面缓存和用户配置，未发现有文档支持的本地行情 API。因此项目不
代理、解密或重放客户端私有连接，也不读取账号配置；可审计的接入路径仍是官方
EMQuant SDK。
