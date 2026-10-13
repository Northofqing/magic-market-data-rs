# target\runtime 脚本恢复记录 — 2026-10-13

> **本文件现随脚本入库**，canonical 位置：
> `tools\runtime\runtime-root\RECOVERY.md`。本目录（`.agent-state\`）是未跟踪的暂存区，
> 不是权威副本；改动请改入库那一份再同步回这里。
>
> 操作说明见同目录 `README.md`。本文件只记录**事故经过与证据**。

## 结论

`target\runtime\` 整目录（脚本 + `bin\` + `tls\` + `client-bundle\`）在 2026-10-13 之前消失。

脚本**部分可恢复**；**密钥类材料最初被判定为不可恢复 —— 该判定是错的，实际全部找回**
（见下方"密钥"一节；错误的成因是只搜了带点的 `C:\Users\13687\.magic-market-data`，
漏掉 Codex 沙箱重定向后的无点路径 `AppData\Local\Packages\...\MagicMarketData`）。

## 恢复来源

`C:\Users\13687\.codex\worktrees\grpc-coverage-20261002\magic-market-data-rs\target\sdk-date-release-20261006\rollback-local\controller-private\`

这是 2026-10-06 那次发布留下的回滚基线，里面保存了 2026-09-23 / 2026-08-13 版本的运行控制脚本。

**关键证据**：该目录的 `keepalive.ps1` 在代码里把原件哈希钉死为"恢复基线"：

```powershell
if ((Get-FileHash -LiteralPath (Join-Path $runtimeRoot 'start.ps1') -Algorithm SHA256).Hash.ToLowerInvariant()
    -cne '39f6279a60da5b411ccb71a86b01f1eaf077bfc6a56d189923395b894b4335d7') {
    throw 'Original launcher differs from recovery baseline'
}
```

恢复到的 `start.ps1` 哈希与之**逐字节一致**，因此它是原始 `target\runtime\start.ps1`，不是仿制品。

## 已恢复（本目录，哈希已二次校验）

| 文件 | 字节 | SHA256 | 说明 |
| --- | ---: | --- | --- |
| `start.ps1` | 7805 | `39f6279a60da5b411ccb71a86b01f1eaf077bfc6a56d189923395b894b4335d7` | 与生产代码内钉死哈希一致 —— **确认原件** |
| `stop.ps1` | 1532 | `a7093cbdd720a4bb78d6d8469c93580db3c9782049c8baea0b9305e13045233e` | 停 agent → 其 monitor 子进程 → gRPC server |
| `keepalive.ps1` | 5486 | `1db332fd2dc32cccddb721ae850d99c6eb01946c0860c62716c49ded467e0e69` | **旧版**（2026-10-03），钉死的是旧二进制哈希 |

## 仍缺失

**脚本（无副本可寻）**
- `status.ps1`
- `configure-remote.ps1` —— `start.ps1` 硬依赖它生成 TLS 材料（报错 `missing TLS file ...; run configure-remote.ps1 first`）
- `limit-pools-window-capture.ps1`
- `probe\probe.ps1`、`probe\report-window.ps1`

**二进制**
- `bin\magic-market-grpc-server.exe`、`bin\magic-market-tdx-agent.exe`
  （回滚基线里的 server 是 2026-09-28 旧版 `517e0b4c…`，**不是**当前合格候选 `9658a0e1…`）

**密钥 —— 已找到，可复制（先前"不可恢复"的结论是错的）**

位置（**Codex 沙箱重定向路径**；`C:\Users\13687\AppData\Local\MagicMarketData` 本身不存在）：

```
C:\Users\13687\AppData\Local\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Local\MagicMarketData\private-runtime\baseline-20261010-1\tls\
```

| 文件 | 字节 | 对应 `target\runtime\tls\` 里的名字 |
| --- | ---: | --- |
| `ca.pem` | 1899 | `ca.pem` |
| `ca-key.pem` | 3272 | （CA 私钥，运行时不需要） |
| `server.pem` | 1801 | `server.pem` |
| `server-key.pem` | 2484 | `server-key.pem` |
| `agent.pem` | 1757 | **`tdx-agent.pem`**（命名不同） |
| `agent-key.pem` | 2484 | **`tdx-agent-key.pem`**（命名不同） |
| `client-ca-bundle.pem` | 3347 | — |
| `mac-client-20261010-v1.pem` | 1749 | — |

`start.ps1` 要求的五个文件（`ca/server/server-key/tdx-agent/tdx-agent-key`）全部有对应来源，
只是 agent 两个需按 `start.ps1` 的命名改名。

**仍未找到**
- `.grpc-token`（`start.ps1` 缺它会**新生成**一个，见脚本逻辑）
- `client-bundle\` 中的 `client.pem` / `client-key.pem`（Mac 客户端证书，应在 Mac 侧）

## 检索范围（均为空）

- 回收站（仅 4 项无关的 Pixelle-Video）
- `C:\DevelopFile\magic-market-data-rs` 全树（含 `.agent-state`、`.worktrees`）
- `C:\Users\13687\.magic-market-data`（仅 `auth-token`、CMS 证书、`bin\`，无 pem）
- `C:\Users\13687\.codex\worktrees` 两个 checkout
- Mac 共享盘 `Z:\`、`V:\`（`Z:\Desktop\Quant` 下只有 `grpc-external-api.md`、`market.proto`）
- `git log --all`：`/target` 自 `.gitignore` 第 1 行起被忽略，**这些脚本从未入库**

## 与合格构建流程的冲突

`run_build.py` 的 `configs_absent()` 在 `target\runtime` 存在时**主动报错拒绝构建**：

```python
if any(fresh_path(path).exists() for path in paths) or fresh_path(REPO / "target/runtime").exists():
    raise RuntimeError("unreviewed Cargo config or existing production runtime")
```

即：**生产运行时目录** 与 **隔离合格构建** 互斥。该构建正是 2026-10-12 产出候选二进制
`9658a0e1…` 的路径，因此"为保证构建通过而清空 `target\runtime`"是目录消失的首要嫌疑。

## 当前实际启动路径（不使用 target\runtime）

今日（2026-10-13 15:46:25）运行的 gRPC/agent/monitor 由
`.agent-state\tdx-persistent-diagnostic-source-20261013-v1\supervisor.py` 拉起，
其根目录是 `C:\Users\13687\.magic-market-data\grpc-joint-debug-20261012-v1\`
（内含 `bin\magic-market-grpc-server.exe` 18388992 B = 合格候选 `9658a0e1…`、`auth-token`）。
该路径**完全不读 `target\runtime`**。

## 迁移执行结果

新运行时根：`C:\Users\13687\.magic-market-data\runtime\`（仓库外，与隔离合格构建不再互斥）。

**已部署**
- `bin\`：server `9658a0e1…`、tdx-agent `3556e7c6…`、monitor-server、native-bridge
- `tls\`：`ca.pem` / `server.pem` / `server-key.pem` / `tdx-agent.pem` / `tdx-agent-key.pem`
  （agent 两个按 `start.ps1` 命名从 `agent.pem` / `agent-key.pem` 改名而来）
- `.grpc-token`：复用 `grpc-joint-debug-20261012-v1\auth-token` 内容，**保住 Mac 侧现有 bearer**
- 脚本：`start.ps1`（钉死哈希 `32310769…`）、`stop.ps1`、`keepalive.ps1`、`status.ps1`、`configure-remote.ps1`

**`start.ps1` 的唯一改动**：删掉 `..\..` 上溯两级找仓库的写法，改为显式参数
`[string] $RepoRoot = "C:\DevelopFile\magic-market-data-rs"`。理由：运行时根已不在仓库内，
上溯两级会指错；`.env.local`（5 个 provider key）必须仍能被找到。其余逐字节保留。

**`configure-remote.ps1` 的行为变更**：只做校验，**拒绝重新生成**。Mac 客户端只信任
`CN=MagicMarketData Recovery CA 20261010 v1` 这一张 CA，重签会在"看起来配置成功"的同时
静默切断 mTLS 信任链。轮换必须与 Mac 侧协同，属另一次显式授权的变更。

**消费方重定向**（原先共 4 处指向 `target\runtime`，gRPC 相关 2 处已改）
| 消费方 | 原路径 | 现路径 |
| --- | --- | --- |
| 计划任务 `MagicMarketGrpcRuntimeKeepAlive` | `<repo>\target\runtime\keepalive.ps1` | `<新根>\keepalive.ps1`（**保持 Disabled**，待发令启用） |
| HKCU Run 键 `MagicMarketData-GrpcRuntime` | `<repo>\target\runtime\keepalive.ps1` | `<新根>\keepalive.ps1` |

keepalive 原先**注册了两遍**（计划任务每 ~2 分钟 + 登录时 Run 键），两处都指向死路径 ——
这是 2026-10-13 之前每两分钟报 `0x8007010B`（ERROR_DIRECTORY）的直接原因。现已一并修正。

**校验通过**
- `configure-remote.ps1`：CA 有效至 2031-10-10，server/agent 至 2027，SAN `magic-market.local`，`Regenerated=$false`
- `keepalive.ps1 -CheckOnly`：`runtime stopped`，exit 3 —— 说明 SID、三个哈希钉、token、
  "无外来运行时"四道闸全部通过
- `status.ps1`：`Healthy=False`（未启动），`TokenPresent=True`，无缺失 TLS

## 启动结果（2026-10-13 14:46Z）

```
GrpcServerPid : 28728
TdxAgentPid   : 19528
Endpoint      : https://10.211.55.3:50051
TlsServerName : magic-market.local
```

`status.ps1` → `Healthy=True`，`ListenerOwnedByServer=True`，`MissingTlsFiles=` 空，exit 0。

**mTLS 在真实握手中验通**：`10.211.55.3:53777`（agent 19528）↔ `10.211.55.3:50051`（server 28728）
ESTABLISHED。agent 以 `--tls-domain magic-market.local` 校验 server 证书并对 server 出示自己的
agent 证书，两端都接受 —— 这比"证书能解析"强，是端到端证据。

server 日志：`server_started ... tls=true reflection=true`，随后
`agent_connected session=2 generation=...:1 watchlist_revision=0 instrument_count=2`。
仅一条 WARN：`agent_disconnected session=1 ...waiting`，1 秒后即以 session=2 重连，属首次握手
时的正常换会话；此后无新 WARN/ERROR/panic。

`MagicMarketGrpcRuntimeKeepAlive` 已 **Enable → Ready**。其 `LastTaskResult=0x8007010B`
是重定向前（22:40:40）那次失败留下的历史值，下一次触发（22:48:48）才会验证新路径。

**未做（决策 A / B 明确暂缓）**
- 未改入库文件 `tools\runtime\windows-autostart.ps1`（该脚本当前未注册到任何计划任务或 Run 键）
- 三个 limit-pools 任务仍指向已不存在的 `target\runtime\`，当前每次触发必然失败：
  `MagicMarketLimitPoolWindowCapture`、`magic-market-limitpools-preopen-probe`、
  `magic-market-limitpools-preopen-report`。其脚本（`limit-pools-window-capture.ps1`、
  `probe\probe.ps1`、`probe\report-window.ps1`）无副本可寻。
