# 构建、部署与运行手册

## 交付物定位

本仓库是 Rust 库工作区，同时提供严格的只读诊断程序；它不是常驻行情守护进程，
也不包含数据库、HTTP 服务或下单功能。生产应用应依赖对应 crate，在自己的服务层
实现限频、缓存、熔断、持久化和授权控制。仓库可打包的二进制用于部署前验证：

- `magic-tdx-live-probe`：TDX 全能力真实探针；
- `magic-emquant-live-probe`：官方 EMQuant SDK 探针；
- `magic-tencent-live-probe`：腾讯 Quote/五档探针；
- `magic-tencent-load-probe`：有界短时并发探针。

## 可重复构建

仓库固定 Rust/Cargo 1.83.0，`Cargo.lock` 也固定 HTTPS 依赖的
URL/IDNA/zeroize 链，避免 Cargo 1.83 解析到 edition-2024 清单。发布构建必须使用
`--locked`，不能删除锁文件后直接升级补丁版本。

```bash
rustup toolchain install 1.83.0 --profile minimal --component rustfmt --component clippy
cargo fetch --locked
bash tools/release/preflight.sh
git commit
bash tools/release/package.sh
```

预检在离线模式运行格式、Rust 1.83 全目标编译、全部测试、严格 Clippy、rustdoc、
doctest、文档链接、合规和 diff 空白检查。打包脚本随后用锁文件构建四个 release
探针，复制为不冲突的文件名，并生成 SHA-256 清单：

```text
target/dist/GIT_SHA/
├── bin/
│   ├── magic-emquant-live-probe[.exe]
│   ├── magic-tdx-live-probe[.exe]
│   ├── magic-tencent-live-probe[.exe]
│   └── magic-tencent-load-probe[.exe]
├── docs/
├── licenses/
├── Cargo.lock
├── CARGO_VERSION
├── README.md
├── RELEASE_REVISION
├── RUSTC_VERSION
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
| TDX | 支持 | 支持 | 支持 | 需要出站 TCP/HTTP 与可写缓存目录 |
| Tencent | 支持 | 支持 | 支持 | Rustls HTTPS 与内置 WebPKI 根证书 |
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
| Tencent | `qt.gtimg.cn:443`，HTTPS | 无持久缓存 |
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

当前开发机已完成短信激活，但真实 SDK 返回 `10001003/EQERR_NO_ACCESS`：账号没有
EMQuant API 产品权限。这不是部署成功状态。开通权限后必须重新运行探针并取得真实
记录；不能把“bridge 能启动”当成数据能力验收。

## 健康检查与上线门

发布包安装后按顺序执行：

```bash
market_release_dir=target/dist/$(git rev-parse HEAD)
"$market_release_dir/bin/magic-tencent-live-probe"
"$market_release_dir/bin/magic-tdx-live-probe"
MAGIC_EMQUANT_BRIDGE=/opt/magic-market-data/libexec/emquant/emquant-snapshot \
  "$market_release_dir/bin/magic-emquant-live-probe"
```

所有探针都以退出码表达真假：预期能力缺记录、代码错配、协议异常、无权限或超时会
退出非零，不会打印模拟记录后成功。TDX 探针理解周末、盘前、午休和盘后差异；
Tencent 盘前零现价会明确失败，涨跌停缺档会标记质量不完整；EMQuant 必须获得所需
API/Level-2 权限才可通过相应数据族。

上线门至少保存以下证据，但不要保存账号、令牌或原始登录包：

- Git SHA、目标三元组、Rust/Cargo 版本和 `SHA256SUMS`；
- Provider、capabilities、证券代码、记录数和探针退出码；
- 每批 `source_at`、`fetched_at/observed_at`、`batch_id` 和质量问题；
- 请求耗时、错误分类、重试/切源次数和最后成功时间；
- EMQuant 权限错误码，但不记录用户名、手机号、密码或 `userInfo` 内容。

生产新鲜度门应比较源时间与采集时间，并按交易阶段设置阈值。没有已验证
`source_at` 的 TDX Quote/盘口不能进入要求源时间的 5 秒链路；Tencent 有源时间但
没有 SLA，仍只能作为补充；EMQuant 的源时间和 Level-2 能力必须在账号授权后验收。

## 常驻服务集成

调用方应在自己的守护进程中持有并复用 Provider client，不要每条记录启动探针。
推荐的最小运行策略：

1. Provider 级并发上限、请求超时和指数退避；
2. 熔断后切源，但在每条记录保留真实 `ProviderId` 和批次证据；
3. 不把旧缓存改写成新源时间，不把缺失值填零；
4. 按证券和数据族监控延迟、空结果、质量降级及源时间倒退；
5. 优雅停机时停止新请求，等待在途批次后再关闭进程。

容器可运行 Core、TDX 和 Tencent，但必须允许上述出站网络并给 TDX 一个可写 HOME。
EMQuant 只有在厂商许可证允许、架构匹配、SDK 能在容器中激活且运行时文件通过秘密
挂载提供时才可容器化；默认发布包不包含这些文件。

## 回滚与升级

每次部署保留上一个完整 `target/dist` 制品和 Git SHA。回滚是把入口切回上一目录，
不复用新版本生成的未验证缓存，并重跑对应 live probe。TDX 服务器缓存可以删除后
重建；业务数据、EMQuant `userInfo` 和厂商 SDK 不应由回滚脚本覆盖。

升级依赖时必须：

1. 在单独提交中更新 `Cargo.lock`；
2. 用 Cargo 1.83.0 运行全工作区 `--locked --offline` 检查；
3. 扫描选中依赖清单，避免再次引入 edition 2024；
4. 运行确定性测试、真实探针和小规模负载探针；
5. 比较能力、字段单位、源时间、错误率和延迟后再放量。

更换网页或私有协议字段不能直接热修上线。先保存合法的最小脱敏 fixture，新增解析
与失败路径测试，再用实盘验证；没有来源证据的字段必须继续 `Unsupported`。
