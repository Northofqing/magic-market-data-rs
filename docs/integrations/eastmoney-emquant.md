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
| 实时 Quote | `csq` / `csqsnapshot` | 待授权登录验证 |
| 日线 | `csc` | 待授权登录验证 |
| 分钟线 | `cmc` / `chmc` | 待授权登录验证 |
| 逐笔 | `chq` | 取决于权限 |
| 盘口/Level-2 | `csqsnapshot` 指标集 | 取决于 Level-2 权限 |
| 资金流 | 指标查询 | 取决于产品权限 |
| 集合竞价 | 实时指标/快照 | 取决于产品权限 |

## 安全边界

- 只接入行情、历史和研究数据；不调用组合、转账、下单、持仓或资金接口。
- `setproxy` 仅用于用户明确配置的合法代理，不做代理抓包或 TLS 绕过。
- 每次返回都必须填充 `provider=Eastmoney`、`observed_at`、批次 ID；只有源明确提供时才填 `source_at`。
- 未授权、权限不足、字段缺失或源时间不可证明时返回结构化错误/`Unsupported`，不能填 0。

## 配置约定（计划）

```text
MAGIC_EMQUANT_LIB=/absolute/path/to/libEMQuantAPIx64.dylib
MAGIC_EMQUANT_SERVER_LIST=/absolute/path/to/ServerList.json.e
MAGIC_EMQUANT_PROXY=host:port        # 可选，仅用户明确配置时启用
```

账号、密码和 API token 不进入仓库、日志或测试 fixture。正式接入前必须先用官方示例
确认登录、Quote 权限和 Level-2 权限，再启用对应 capability。

本地 SDK 文件布局和示例语法可用以下只读检查验证：

```text
bash tools/emquant/check_sdk.sh /path/to/EMQuantAPI_CPP_Mac
```
