# 构建、部署与运行手册

## 交付物定位

本仓库是 Rust 库工作区，同时提供严格的只读诊断程序；它不是常驻行情守护进程，
也不包含数据库、HTTP 服务或下单功能。生产应用应依赖对应 crate，在自己的服务层
实现限频、缓存、熔断、持久化和授权控制。仓库可打包的二进制用于部署前验证：

- `magic-tdx-live-probe`：TDX 全能力真实探针；
- `magic-emquant-live-probe`：官方 EMQuant SDK 探针；
- `magic-tencent-live-probe`：腾讯 Quote/五档/K线/分时/逐笔/市场统计探针；
- `magic-tencent-load-probe`：有界短时并发探针；
- `magic-sina-live-probe`：新浪 Quote/五档/K线/最新分时/财务三表/ETF 期权探针；
- `magic-sina-load-probe`：最多 40 请求/4 线程的有界短时并发探针；
- `magic-eastmoney-{live,load}-probe`：东财公开研究、资金、信号、资本、打板和未准入诊断；
- `magic-cninfo-{live,load}-probe`：巨潮公告/PDF metadata 和互动易；
- `magic-ths-{live,load}-probe`：同花顺一致预期、强势原因、涨停池和热榜；
- `magic-cls-{live,load}-probe`：财联社签名全球电报；
- `magic-jin10-{live,load}-probe`：金十公开 7x24 财经快讯；
- `magic-thepaper-{live,load}-probe`：澎湃财经频道原生文章；
- `magic-yonhap-{live,load}-probe`：韩联社 7 路简体中文 RSS metadata 诊断；
- `magic-wallstreetcn-{live,load}-probe`：华尔街见闻单一公开 RSS metadata；
- `magic-baidu-{live,load}-probe`：百度未复权日 K 和源端 MA；
- `magic-iwencai-{live,load}-probe`：需要授权 API Key 的语义搜索；
- `magic-exchange-{live,load}-probe`：SSE/SZSE 公告与龙虎榜、SZSE Quote/五档、
  HKEX 北向日统计；CFFEX 股指期货交割通知当前仅提供未准入诊断入口；
- `magic-gov-live-probe`：国务院政策库官方文件；
- `magic-nbs-live-probe`：国家统计局有界诊断；
- `magic-pbc-{live,load}-probe`：人民银行已准入 2024 货币供应量验证；
- `magic-cfets-{live,load}-probe`：已准入 Shibor、LPR 和官方汇率验证；
- `magic-fred-{live,load}-probe`：需要运行时 `FRED_API_KEY` 的 FRED 官方序列；
- `magic-imf-{live,load}-probe`：IMF DataMapper 官方序列；
- `magic-worldbank-live-probe`：World Bank 指标与结构化 unit 阻断诊断；
- `magic-sec-{live,load}-probe`：需要描述性 `SEC_USER_AGENT` 的 EDGAR 元数据；
- `magic-xinhua-{live,load}-probe`：新华财经首屏 metadata；
- `magic-yicai-{live,load}-probe`：第一财经首屏 metadata；
- `magic-stcn-{live,load}-probe`：证券时报人民财讯首屏 metadata；
- `magic-router-live-probe`：TDX→Tencent 证据门与切源探针。

## 可重复构建

仓库不固定具体 Rust/Cargo 版本。开发与发布使用运行主机的默认工具链，CI 使用当前
stable；发布包会保存实际 `rustc -vV` 与 `cargo -V` 输出。`Cargo.lock` 固定依赖
解析结果，发布构建必须使用 `--locked`，不能删除锁文件后直接升级依赖。

```bash
cargo fetch --locked
bash tools/release/preflight.sh
git commit
bash tools/release/package.sh
```

预检先打印当前工具链版本，再在每次新建的隔离 target 目录中，以离线模式运行格式、
全目标编译、全部测试、严格 Clippy、rustdoc、doctest、文档链接、合规和 diff
空白检查，避免旧元数据污染门禁。脚本不安装或切换工具链。打包脚本随后用锁文件
构建四十八个 release 探针，复制为不冲突的文件名，并生成 SHA-256 清单。这里描述
可重复流程，不自动证明任意未来工作树已经通过 release gate；当前合并版本的实际
门禁和覆盖率证据记录在根目录 README 的“当前验收状态”：

```text
target/dist/GIT_SHA/
├── bin/
│   ├── magic-baidu-live-probe[.exe]
│   ├── magic-baidu-load-probe[.exe]
│   ├── magic-cls-live-probe[.exe]
│   ├── magic-cls-load-probe[.exe]
│   ├── magic-cninfo-live-probe[.exe]
│   ├── magic-cninfo-load-probe[.exe]
│   ├── magic-emquant-live-probe[.exe]
│   ├── magic-eastmoney-live-probe[.exe]
│   ├── magic-eastmoney-load-probe[.exe]
│   ├── magic-exchange-live-probe[.exe]
│   ├── magic-exchange-load-probe[.exe]
│   ├── magic-gov-live-probe[.exe]
│   ├── magic-iwencai-live-probe[.exe]
│   ├── magic-iwencai-load-probe[.exe]
│   ├── magic-jin10-live-probe[.exe]
│   ├── magic-jin10-load-probe[.exe]
│   ├── magic-nbs-live-probe[.exe]
│   ├── magic-pbc-live-probe[.exe]
│   ├── magic-pbc-load-probe[.exe]
│   ├── magic-cfets-live-probe[.exe]
│   ├── magic-cfets-load-probe[.exe]
│   ├── magic-fred-live-probe[.exe]
│   ├── magic-fred-load-probe[.exe]
│   ├── magic-imf-live-probe[.exe]
│   ├── magic-imf-load-probe[.exe]
│   ├── magic-worldbank-live-probe[.exe]
│   ├── magic-sec-live-probe[.exe]
│   ├── magic-sec-load-probe[.exe]
│   ├── magic-xinhua-live-probe[.exe]
│   ├── magic-xinhua-load-probe[.exe]
│   ├── magic-yicai-live-probe[.exe]
│   ├── magic-yicai-load-probe[.exe]
│   ├── magic-stcn-live-probe[.exe]
│   ├── magic-stcn-load-probe[.exe]
│   ├── magic-router-live-probe[.exe]
│   ├── magic-sina-live-probe[.exe]
│   ├── magic-sina-load-probe[.exe]
│   ├── magic-tdx-live-probe[.exe]
│   ├── magic-tencent-live-probe[.exe]
│   ├── magic-tencent-load-probe[.exe]
│   ├── magic-thepaper-live-probe[.exe]
│   ├── magic-thepaper-load-probe[.exe]
│   ├── magic-ths-live-probe[.exe]
│   ├── magic-ths-load-probe[.exe]
│   ├── magic-yonhap-live-probe[.exe]
│   ├── magic-yonhap-load-probe[.exe]
│   ├── magic-wallstreetcn-live-probe[.exe]
│   └── magic-wallstreetcn-load-probe[.exe]
├── docs/
├── licenses/
├── Cargo.lock
├── CARGO_VERSION
├── README.md
├── RELEASE_REVISION
├── RUSTC_VERSION
├── rust-toolchain.toml
├── TARGET_TRIPLE
└── SHA256SUMS
```

打包脚本拒绝有未提交的 tracked 改动、未跟踪的源码/发布脚本/Cargo 配置，并且只
复制 Git 已跟踪的文档，不会把本机草稿或未跟踪文件混入制品。该目录是当前操作
系统和 CPU 架构的产物，不能跨平台复制执行。构建使用每次新建的隔离 target 目录
和显式 `TARGET_TRIPLE`，不会从常规 `target/release` 复制旧二进制。部署时先核对
`RELEASE_REVISION` 与批准的 Git SHA，再在目标机执行 SHA-256 校验。POSIX 发布
脚本需要 Bash 与常用 Unix 工具；Windows 原生制品应在 Git Bash 或 Windows CI
中生成。WSL 只能生成 Linux 制品，不能冒充 Windows 原生构建。

在目标机校验完整制品（macOS 使用 `shasum`，Linux 通常使用 `sha256sum`）：

```bash
cd target/dist/GIT_SHA
shasum -a 256 -c SHA256SUMS
```

## 平台矩阵

| 组件 | macOS | Linux | Windows | 说明 |
| --- | --- | --- | --- | --- |
| `magic-market-core` | 支持 | 支持 | 支持 | 纯 Rust 合同 |
| `magic-market-router` | 支持 | 支持 | 支持 | 纯 Rust 同步路由与证据检查 |
| `magic-market-transport` 与新官方数据源 | 支持 | 支持 | 支持 | Reqwest/Rustls HTTPS；PBC、CFETS 和三家新闻按 family 已准入，其余保持显式诊断/关闭 |
| TDX | 支持 | 支持 | 支持 | 需要出站 TCP/HTTP 与可写缓存目录 |
| Tencent | 支持 | 支持 | 支持 | Rustls HTTPS 与内置 WebPKI 根证书 |
| Sina | 支持 | 支持 | 支持 | Rustls HTTPS、GB18030/JSON，无本地运行时 |
| Eastmoney/CNInfo/THS/CLS/Jin10/The Paper/Yonhap/WallstreetCN/Baidu | 支持 | 支持 | 支持 | Rustls HTTPS；公共网页补充源；Yonhap 当前 live TLS 未准入，WallstreetCN 已准入 |
| State Council official | 支持 | 支持 | 支持 | Rustls HTTPS；官方政策文件 |
| SSE/SZSE/HKEX official | 支持 | 支持 | 支持 | Rustls HTTPS；官方公共只读数据 |
| CFFEX official diagnostic | 仅诊断 | 仅诊断 | 仅诊断 | capability=false；生产 trait `Unsupported`；官方 TLS live 未通过 |
| iWencai | 支持 | 支持 | 支持 | Rustls HTTPS；需要获授权 API Key |
| EMQuant Rust 层 | 支持 | 可编译 | 可编译 | 运行还取决于厂商 SDK |
| 当前 EMQuant C++ bridge | x86_64 macOS | 未适配 | 未适配 | 使用 `.dylib`、`dlopen` 和 POSIX API |

macOS 的 EMQuant SDK 必须与进程架构相同。Apple Silicon 主机若只有厂商 x86_64
SDK，需要在 x86_64/Rosetta 构建和运行整条链路，不能让 arm64 Rust 进程加载 x86_64
动态库。Linux/Windows 部署 EMQuant 前必须基于对应平台官方 SDK 单独实现并验收
桥接器；当前包不能声称已经跨平台运行 EMQuant。

## 网络与文件权限

| Provider | 必需出站访问 | 本地写入 |
| --- | --- | --- |
| TDX | 已配置行情服务器 TCP 7709；财务包 `data.tdx.com.cn:80` | `~/.tdxrs/server_cache.json`；调用方指定的财务缓存 |
| Tencent | `qt.gtimg.cn:443`、`web.ifzq.gtimg.cn:443`、`ifzq.gtimg.cn:443`、`stock.gtimg.cn:443`，HTTPS | 无持久缓存 |
| Sina | `hq.sinajs.cn:443`、`quotes.sina.cn:443`、`stock.finance.sina.com.cn:443`，HTTPS；全球指数/汇率也使用 `hq.sinajs.cn` | 无持久缓存 |
| Eastmoney Web | 集成文档白名单中的 `eastmoney.com`/`dfcfw.com` 主机（含 `pdf.dfcfw.com`），HTTPS 443 | 无持久缓存 |
| CNInfo | `www.cninfo.com.cn:443`、`irm.cninfo.com.cn:443`、`static.cninfo.com.cn:443` | 仅 24 小时进程内 org 映射缓存 |
| THS | `basic`、`zx`、`data`、`dq.10jqka.com.cn:443` | 无持久缓存 |
| CLS | `www.cls.cn:443` | 无持久缓存 |
| Jin10 | `flash-api.jin10.com:443` | 无持久缓存 |
| The Paper | `www.thepaper.cn:443` | 无持久缓存 |
| Yonhap | `cn.yna.co.kr:443`，仅 `/RSS/news.xml`、`/RSS/politics.xml`、`/RSS/economy.xml`、`/RSS/society.xml`、`/RSS/culture-sports.xml`、`/RSS/nk.xml`、`/RSS/china-relationship.xml` | 无持久缓存；不抓文章页 |
| WallstreetCN | `dedicated.wallstreetcn.com:443`，仅精确 `/rss.xml` | 无持久缓存；不抓文章页、description 或正文 |
| Baidu | `finance.pae.baidu.com:443` | 无持久缓存 |
| SSE/SZSE/HKEX official | `query.sse.com.cn:443`、`www.szse.cn:443`、`www.hkex.com.hk:443` | 无持久缓存 |
| CFFEX diagnostic | `www.cffex.com.cn:443` | 无持久缓存；仅有界显式 probe |
| State Council | `sousuo.www.gov.cn:443`；返回链接仅允许 `www.gov.cn:443` | 无持久缓存 |
| NBS | `www.stats.gov.cn:443` | 无持久缓存；landing 可访问，但机器序列合同未证明，只有显式诊断 |
| PBC | `www.pbc.gov.cn:443`，仅精确编目 HTML | 无持久缓存 |
| CFETS | `www.chinamoney.com.cn:443`，仅 `/ags/ms/` 下已审计 JSON | 无持久缓存 |
| FRED | `api.stlouisfed.org:443` | `FRED_API_KEY` 只由环境/secret 注入，不落盘或进入日志 |
| IMF | `www.imf.org:443`，仅 DataMapper API v2 | 无持久缓存 |
| World Bank | `api.worldbank.org:443`，仅 v2 indicator/country | 无持久缓存 |
| SEC EDGAR | `data.sec.gov:443`，仅 submissions JSON | `SEC_USER_AGENT` 只由环境/secret 注入；不抓 Archives 内容 |
| Xinhua Finance | `www.cnfin.com:443`，仅 `/news/index.html` | 无持久缓存；不抓文章页 |
| Yicai | `www.yicai.com:443`，仅 `/news/info/` | 无持久缓存；不抓文章页 |
| Securities Times | `www.stcn.com:443`，仅 `type=kx` 首屏 XHR | 无持久缓存；不抓文章页 |
| iWencai | `openapi.iwencai.com:443` | API Key 仅由环境/秘密挂载提供，不落盘 |
| EMQuant | 厂商 `ServerList.json.e` 定义的目标 | bridge 同级 `runtime/` 与权限 0600 的 `userInfo` |

防火墙应只开放所需出站目标。TDX 财务下载当前是厂商 HTTP 分发端点，代码通过响应
边界、ZIP 目录、解压大小和 CRC 校验内容，但传输层不加密；对传输保密或更严格供应
链有要求的环境应关闭该能力或经批准的完整性代理接入，不能把它描述成 HTTPS。

运行服务账号必须有独立可写 HOME，TDX SmartClient 才能安全保存服务器健康缓存。
不要让多个不可信账号共享该目录。容器内应挂载专用可写目录并显式设置 HOME；根
文件系统可保持只读。

## EMQuant 运行时部署

厂商 SDK、服务器列表、激活令牌受其许可证约束，不进入 Git，也不由发布包自动
分发。在每台获授权的目标机上准备 SDK：

```bash
bash tools/emquant/check_sdk.sh /approved/path/EMQuantAPI_CPP_Mac
bash tools/emquant/build_snapshot_bridge.sh /approved/path/EMQuantAPI_CPP_Mac
```

脚本创建下列项目内、Git 忽略布局：

```text
target/emquant/
├── emquant-snapshot
└── runtime/
    ├── libEMQuantAPIx64.dylib
    ├── ServerList.json.e
    ├── loginactivator_mac
    ├── image/
    └── userInfo                 # 激活后生成，0600
```

macOS 脚本只清除复制件的隔离属性并对复制件临时签名，不修改 Downloads 中的厂商
原文件。首次部署要在目标机运行 `runtime/loginactivator_mac` 完成短信激活；桌面
东方财富客户端登录不能替代 API 激活。`userInfo` 通常与设备绑定：备份只用于同机
灾难恢复且必须加密，迁移主机应重新激活，不应把它复制进镜像、制品、日志或 Git。

若把 bridge 安装到仓库外，必须保持 `runtime/` 与 bridge 同级，并给 Rust 进程设置：

```text
MAGIC_EMQUANT_BRIDGE=/opt/magic-market-data/libexec/emquant/emquant-snapshot
```

只有覆盖同级默认布局时才设置 `MAGIC_EMQUANT_LIB` 和
`MAGIC_EMQUANT_SERVER_LIST`。旧版账号密码变量必须成对注入秘密管理器，不能写进
shell history、服务文件或镜像：

```text
MAGIC_EMQUANT_USERNAME
MAGIC_EMQUANT_PASSWORD
```

当前开发机在 2026-07-23 重新完成短信激活并开通 Choice 权限后，官方 SDK 已成功
登录，`csd` 日线和 `css` 日级资金流取得真实记录。Quote、五档和 `chmc` 分钟线仍
返回 `10001012/EQERR_ACCESS_INSUFFICIENCE`。该码表示账号已认证，但具体服务或字段
权限不足；需要分别追加对应权限并重跑数据族探针。不能用“审核通过”“登录成功”
或某一个数据族成功推断其余数据族已经上线。

## 健康检查与上线门

发布包安装后按顺序执行：

```bash
market_release_dir=target/dist/$(git rev-parse HEAD)
"$market_release_dir/bin/magic-tencent-live-probe"
"$market_release_dir/bin/magic-sina-live-probe"
"$market_release_dir/bin/magic-tdx-live-probe"
"$market_release_dir/bin/magic-router-live-probe"
"$market_release_dir/bin/magic-eastmoney-live-probe"
"$market_release_dir/bin/magic-cninfo-live-probe"
"$market_release_dir/bin/magic-ths-live-probe"
"$market_release_dir/bin/magic-cls-live-probe"
"$market_release_dir/bin/magic-jin10-live-probe"
"$market_release_dir/bin/magic-thepaper-live-probe"
"$market_release_dir/bin/magic-yonhap-live-probe"
"$market_release_dir/bin/magic-wallstreetcn-live-probe"
"$market_release_dir/bin/magic-baidu-live-probe"
"$market_release_dir/bin/magic-exchange-live-probe"
"$market_release_dir/bin/magic-gov-live-probe"
"$market_release_dir/bin/magic-nbs-live-probe"
"$market_release_dir/bin/magic-pbc-live-probe"
"$market_release_dir/bin/magic-cfets-live-probe" \
  2026-07-20 2026-07-29
"$market_release_dir/bin/magic-imf-live-probe"
"$market_release_dir/bin/magic-worldbank-live-probe" --diagnostic
"$market_release_dir/bin/magic-xinhua-live-probe"
"$market_release_dir/bin/magic-yicai-live-probe"
"$market_release_dir/bin/magic-stcn-live-probe"

# 仅在运行机获合法配置时执行；不要把值写入日志：
FRED_API_KEY=... "$market_release_dir/bin/magic-fred-live-probe"
SEC_USER_AGENT='application/version operator-contact' \
  "$market_release_dir/bin/magic-sec-live-probe"
MAGIC_TENCENT_LOAD_OPERATION=mixed MAGIC_TENCENT_LOAD_REQUESTS=20 \
  MAGIC_TENCENT_LOAD_CONCURRENCY=4 \
  "$market_release_dir/bin/magic-tencent-load-probe"
MAGIC_TENCENT_LOAD_OPERATION=statistics MAGIC_TENCENT_LOAD_REQUESTS=12 \
  MAGIC_TENCENT_LOAD_CONCURRENCY=3 \
  "$market_release_dir/bin/magic-tencent-load-probe"
MAGIC_SINA_LOAD_OPERATION=mixed MAGIC_SINA_LOAD_REQUESTS=20 \
  MAGIC_SINA_LOAD_CONCURRENCY=4 \
  "$market_release_dir/bin/magic-sina-load-probe"
MAGIC_SINA_LOAD_OPERATION=financial MAGIC_SINA_LOAD_REQUESTS=6 \
  MAGIC_SINA_LOAD_CONCURRENCY=2 \
  "$market_release_dir/bin/magic-sina-load-probe"
MAGIC_SINA_LOAD_OPERATION=options MAGIC_SINA_LOAD_REQUESTS=6 \
  MAGIC_SINA_LOAD_CONCURRENCY=2 \
  MAGIC_SINA_OPTION_UNDERLYING=510050 \
  MAGIC_SINA_OPTION_SAMPLE_CONTRACTS=2 \
  "$market_release_dir/bin/magic-sina-load-probe"
MAGIC_EMQUANT_BRIDGE=/opt/magic-market-data/libexec/emquant/emquant-snapshot \
  "$market_release_dir/bin/magic-emquant-live-probe"

MAGIC_EASTMONEY_LOAD_REQUESTS=6 MAGIC_EASTMONEY_LOAD_CONCURRENCY=1 \
  "$market_release_dir/bin/magic-eastmoney-load-probe"
MAGIC_CNINFO_LOAD_REQUESTS=3 MAGIC_CNINFO_LOAD_CONCURRENCY=1 \
  "$market_release_dir/bin/magic-cninfo-load-probe"
MAGIC_THS_LOAD_REQUESTS=3 MAGIC_THS_LOAD_CONCURRENCY=1 \
  "$market_release_dir/bin/magic-ths-load-probe"
MAGIC_CLS_LOAD_REQUESTS=2 MAGIC_CLS_LOAD_CONCURRENCY=1 \
  "$market_release_dir/bin/magic-cls-load-probe"
MAGIC_JIN10_LOAD_REQUESTS=2 \
  "$market_release_dir/bin/magic-jin10-load-probe"
MAGIC_THEPAPER_LOAD_REQUESTS=2 \
  "$market_release_dir/bin/magic-thepaper-load-probe"
MAGIC_YONHAP_LOAD_REQUESTS=2 \
  "$market_release_dir/bin/magic-yonhap-load-probe"
MAGIC_WALLSTREETCN_LOAD_REQUESTS=2 \
  "$market_release_dir/bin/magic-wallstreetcn-load-probe"
MAGIC_BAIDU_LOAD_REQUESTS=2 MAGIC_BAIDU_LOAD_CONCURRENCY=1 \
  "$market_release_dir/bin/magic-baidu-load-probe"
MAGIC_EXCHANGE_LOAD_REQUESTS=8 MAGIC_EXCHANGE_LOAD_CONCURRENCY=1 \
"$market_release_dir/bin/magic-exchange-load-probe"
"$market_release_dir/bin/magic-pbc-load-probe"
"$market_release_dir/bin/magic-cfets-load-probe" \
  2026-07-20 2026-07-29
FRED_API_KEY=... "$market_release_dir/bin/magic-fred-load-probe"
"$market_release_dir/bin/magic-imf-load-probe"
SEC_USER_AGENT='application/version operator-contact' \
  "$market_release_dir/bin/magic-sec-load-probe"
"$market_release_dir/bin/magic-xinhua-load-probe"
"$market_release_dir/bin/magic-yicai-load-probe"
"$market_release_dir/bin/magic-stcn-load-probe"

# 只有已配置授权 Key 的环境才运行：
MAGIC_IWENCAI_API_KEY=... \
  "$market_release_dir/bin/magic-iwencai-live-probe"
```

期权 load probe 默认在同次运行开始时自动发现当前合约，发现步骤不计入负载耗时。
需要复测指定合约时，可用 `MAGIC_SINA_OPTION_CONTRACTS` 传入逗号分隔代码；代码
不会回退到可能已经到期的固定合约。

所有探针都以退出码表达真假：预期能力缺记录、代码错配、协议异常、无权限或超时会
退出非零，不会打印模拟记录后成功。TDX 探针理解周末、盘前、午休和盘后差异；
Tencent 盘前零现价会明确失败，涨跌停缺档会标记质量不完整，load probe 会轮转
Quote、日线、分时、当日逐笔和市场统计；router probe 必须打印 TDX 的失败/质量拒绝、
Tencent 的选中状态和真实 Quote；EMQuant 当前会打印真实日线和资金流，但因
Quote/Level-2/分钟权限不足而保持整体非零退出；Sina probe 会打印六类 K 线、
三市场五档、最新交易日分时、三张财务报表，以及已实测 510050 的 ETF 期权合约、
最优买卖一档 T 型报价和希腊字母；日线成交额和涨跌停空侧保持缺失。路由探针没有
缓存或跨源拼接，两个来源都失败时退出非零。公共研究/内容 Provider 同样要求非空
严格批次；Eastmoney 已声明能力的完整 live/mixed probe 必须为零退出，两个未声明
资金流端点若继续返回 empty reply 则单独打印预期失败诊断，不能登记为资金流实盘
通过；东财最新财经资讯必须校验完整滚动首屏，关键词搜索则因无结构化证券身份保持
未准入。交易所官方 probe 要求公告
证券/日期及分页匹配、龙虎榜证券/交易日和完整买五卖五匹配、SZSE Quote/盘口身份及
源时间匹配、HKEX 两通道与 Top10 完整；任一来源失败时整体非零。Jin10 probe 只
接收未锁定的公开 type-0/type-2 新闻，The Paper probe 只接收财经频道原生文章并
排除外链转载；两者都不从文本猜测证券身份。Yonhap probe 只读取选定的官方中文
RSS，打印标题、ACK ID、规范链接、来源时间、频道和证据，并验证 summary/content
缺失；截至 2026-07-26 Rolling/Economy 的生产 Rust TLS 都收到 unexpected EOF，
所以该 probe 预期非零、capability 保持 false。`MAGIC_YONHAP_MATCH` 的未命中只
表示当前有界 RSS 窗口没有该标题文本，不能作为历史不存在的证据。WallstreetCN
probe 只读取精确 `/rss.xml`，严格解析完整 feed 后打印标题、数字 ID、规范链接、
来源时间和证据，summary/content 恒缺失；2026-07-26 live 20 条和同一客户端串行
load 2/2 已通过，`global_news=true`。`MAGIC_WALLSTREETCN_MATCH` 同样只是当前
有界 feed 的本地标题匹配。iWencai 无授权 Key 时预期
返回脱敏鉴权错误，不能把这次运行登记为语义搜索实盘通过。

上线门至少保存以下证据，但不要保存账号、令牌或原始登录包：

- Git SHA、目标三元组、Rust/Cargo 版本和 `SHA256SUMS`；
- Provider、capabilities、证券代码、记录数和探针退出码；
- 每批 `source_at`、`fetched_at/observed_at`、`batch_id` 和质量问题；
- 请求耗时、错误分类、重试/切源次数和最后成功时间；
- EMQuant 权限错误码，但不记录用户名、手机号、密码或 `userInfo` 内容。

生产新鲜度门应比较源时间与采集时间，并按交易阶段设置阈值。没有已验证
`source_at` 的 TDX Quote/盘口不能进入要求源时间的 5 秒链路；Tencent 有源时间但
没有 SLA，仍只能作为补充；Sina 同样有源时间但没有 SLA，Quote 与 K 线也不是原子
快照；EMQuant 的源时间和 Level-2 能力必须在账号授权后验收。

## 常驻服务集成

调用方应在自己的守护进程中持有并复用 Provider client，不要每条记录启动探针。
推荐的最小运行策略：

1. Provider 级并发上限、请求超时和指数退避；
2. 熔断后切源，但在每条记录保留真实 `ProviderId` 和批次证据；
3. 不把旧缓存改写成新源时间，不把缺失值填零；
4. 按证券和数据族监控延迟、空结果、质量降级及源时间倒退；
5. 优雅停机时停止新请求，等待在途批次后再关闭进程。

`magic-market-router` 可以实现第 2 项的顺序切源和 attempt trace，但不负责定时
调度、缓存、持久化或熔断状态机。调用方必须按
[`MULTI_PROVIDER_ROUTING.md`](MULTI_PROVIDER_ROUTING.md) 显式分类各 Provider
错误，并根据业务数据族选择完整性和来源时间门。

容器可运行 Core、Router 和全部纯 Rust Provider，但必须允许上述出站网络并给 TDX
一个可写 HOME。公开网页 Provider 不需要复制桌面客户端文件；iWencai Key 必须通过
只读 secret 注入，禁止写入镜像、命令历史或 probe 日志。
EMQuant 只有在厂商许可证允许、架构匹配、SDK 能在容器中激活且运行时文件通过秘密
挂载提供时才可容器化；默认发布包不包含这些文件。

## 回滚与升级

每次部署保留上一个完整 `target/dist` 制品和 Git SHA。回滚是把入口切回上一目录，
不复用新版本生成的未验证缓存，并重跑对应 live probe。TDX 服务器缓存可以删除后
重建；业务数据、EMQuant `userInfo` 和厂商 SDK 不应由回滚脚本覆盖。

升级依赖时必须：

1. 在单独提交中更新 `Cargo.lock`；
2. 用当前默认 Cargo 运行全工作区 `--locked --offline` 检查并记录版本；
3. 扫描选中依赖清单和安全公告，并确认其编译器要求符合当前 stable；
4. 运行确定性测试、真实探针和小规模负载探针；
5. 比较能力、字段单位、源时间、错误率和延迟后再放量。

更换网页或私有协议字段不能直接热修上线。先保存合法的最小脱敏 fixture，新增解析
与失败路径测试，再用实盘验证；没有来源证据的字段必须继续 `Unsupported`。
