# WinVerifyTrustEx / WSS index 未写回：第一方证据与单符号对照边界

日期：2026-10-11。性质：只读 PRIMARY 来源研究；不是修复、SOURCE_ONLY 新候选、native 重试或任何 GO。仅新增本 Markdown，不改旧 packet、正式源码、flag、index 接受条件或 alias 约定。

## 结论

**可以把“仅将 API 符号 WinVerifyTrustEx 换为 WinVerifyTrust”列为下一项待单独批准的源码诊断对照，但不能称为已证实的修复。** 两个 subject 的相同 index 现象使共享调用入口值得隔离；Microsoft 的 WSS 文档和 CodeSigning FILE 样例指向 WinVerifyTrust。然而，所读第一方材料没有给出“Ex 不写回、非 Ex 才写回”或两者实现等价的契约。官方样例也没有实际检验 `dwVerifiedSigIndex` 的写回。[WSS 文档](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_signature_settings)、[固定 CodeSigning 源码](https://github.com/microsoft/Windows-classic-samples/blob/434f6002bdf9cf9829406c3ff2b33387982d6168/Samples/Security/CodeSigning/cpp/codesigning.cpp#L40)、[Ex 文档](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrustex)。

关键纠正：固定 SDK `WinTrust.h` 465–468 行的 “use WinVerifyTrust() instead” 承接的是 **Ex 返回值曾声明为 HRESULT、不得使用 SUCCEEDED/FAILED** 的问题，不是参数复制、不修改参数或不支持 WSS 的警告。不能把该注释当作本次根因证据。本机固定 SDK（`C:/Program Files (x86)/Windows Kits/10/Include/10.0.26100.0/um/WinTrust.h:465`）、[Ex 返回值说明](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrustex#return-value)。

## 1. 实际公共输出：同现象，不是同原因

本研究完整读取 FILE 新决策及三个原始公共输出，并完整读取 ROOT 指定的 CAT receipt 与已关闭失败记录；全部原始字节 pins 见第 5 节。没有重新执行它们。

| 原始字段 | FILE：固定公共 app Python | CAT：原固定公共目录成员诊断 |
| --- | --- | --- |
| requested / initial sentinel | 0 / 4294967295 | 0 / 4294967295（ROOT 已关闭记录） |
| trust / CLOSE | 0 / 0 | 0 / 0 |
| state_present | true | true |
| returned_index | 4294967295，即 FFFFFFFF | 4294967295，即 FFFFFFFF |
| custody_rounds / cleanup | 3 / [] | 3 / [] |
| failure | TRUST_VERIFY / INDEX_MISMATCH / winerror 0 | TRUST_VERIFY / INDEX_MISMATCH / winerror 0 |
| 成功/资格 | control_candidate_passed=false；六 GO=false；provenance=null | signature_evidence_gathered=false；未到达 selected leaf / Microsoft policy；ROOT 资格与 GO=false |
| 进程终态的证据层级 | ROOT 本次消息报告 df7bbc 自然 exit 1；本研究未直接观察进程 | ROOT 原记录明确 e9af71 自然 wrapper exit 1、非 forced/watchdog |

来源：FILE receipt（`C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/receipt.json`）、FILE 新决策（`C:/DevelopFile/magic-market-data-rs/.agent-state/root-file-wss-v2-one-public-native-decision-20261011-v1.json`）、CAT receipt（`C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-ssh-catalog-state-index-diagnostic-source-20261011-v1/receipt.json`）、CAT ROOT 原失败记录（`C:/DevelopFile/magic-market-data-rs/.agent-state/root-fixed-ssh-state-index-failed-diagnostic-result-20261011-v1.json`）。receipt 本身明确 `receipt_alone_authoritative=false`，不能单凭它推断自然退出或完整资格。

FILE journal 为 PUBLIC_BOUND=true、CONTROL_RETURN=false、POST_CUSTODY=true；reservation 保持六 GO=false/null provenance。FILE 负例没有将原 CAT 负例改为通过。journal（`C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/journal.jsonl`）、reservation（`C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/reservation.json`）、ROOT 决策 interpretation（`C:/DevelopFile/magic-market-data-rs/.agent-state/root-file-wss-v2-one-public-native-decision-20261011-v1.json`）。

本次完整重读并 fresh pin 的 FILE adapter 明确使用 System32 `wintrust.dll` 的 `WinVerifyTrustEx`；restype 为 `c_int32`，参数为 HWND 指针、GUID 指针、TRUST_DATA 指针。结构守卫为 FILE_INFO=32、SIGNATURE_SETTINGS=32、TRUST_DATA=88，逐字段 offsets 明确检查；settings=(32, index 0, flag 1, secondary 0, sentinel FFFFFFFF, NULL policy)；UI=2、revocation=1、provider flags=0x3040、HWND=-1。FILE 借用 held Python handle，不重开；VERIFY 与 finally CLOSE 使用同一 action/data。index 读取来自原 `settings.dwVerifiedSigIndex`，在 trust=0 后与 state 一并检查；sentinel 未被改写即失败。固定 FILE adapter（`C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/file_adapter.prepare.py`）。

**有限推论**：FILE 的 TRUST_DATA 原先零初始化、receipt 却报告 state_present=true，因此观察不支持“Ex 完全没有任何参数输出变化”的概括。它仅显示这一次原 settings 的 index 仍是 sentinel；不能说明为什么，也不能证明特定 signature 实际被选中。此为源码与输出合并后的推论，不是新的 OS 试验。adapter（`C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/file_adapter.prepare.py`）、receipt（`C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/receipt.json`）。

## 2. API 与参数修改：官方材料实际说了什么

### 2.1 两个入口的已知语义

固定 SDK 462–468 行分别声明 `LONG WINAPI WinVerifyTrust(HWND, GUID*, LPVOID)` 与 `long WINAPI WinVerifyTrustEx(HWND, GUID*, WINTRUST_DATA*)`。Learn 的两个页面都将调用描述为按 action identifier 交给支持该 action 的 trust provider；二者第三参数都标为 `[in]`，Ex 的参数类型更具体。SDK（`C:/Program Files (x86)/Windows Kits/10/Include/10.0.26100.0/um/WinTrust.h:462`）、[WinVerifyTrust](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrust)、[WinVerifyTrustEx](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrustex)。

WinVerifyTrust 明确只把 0 视为成功，返回值不是 HRESULT；Ex 文档同样要求按 Win32 error code 测试，不用 HRESULT 成功宏。现 adapter 的 signed32 / ==0 不应因换符号而改为 HRESULT 成功判断。Learn Ex 页面正文仍保留“declared as HRESULT”的历史措辞，但语法与固定 SDK 当前声明是 long；这里的差异不能推导出 writeback 差异。[WinVerifyTrust 返回值](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrust#return-value)、[Ex 返回值](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrustex#return-value)、SDK 465 行（`C:/Program Files (x86)/Windows Kits/10/Include/10.0.26100.0/um/WinTrust.h:465`）。

本次所读两份 API 页面及其固定 commit 原始 Markdown **没有** 声明 Ex 复制输入结构、禁止输出修改、忽略 WSS、保证保留 index，或保证和 WinVerifyTrust 是同一实现/alias。这个结论只限所读材料，不是对所有 Microsoft 文档的穷尽证明。[固定 Ex 文档源码](https://github.com/MicrosoftDocs/sdk-api/blob/656e9656bfaef34dd29cd7ee9e1146aec17e924c/sdk-api-src/content/wintrust/nf-wintrust-winverifytrustex.md)、[固定非 Ex 文档源码](https://github.com/MicrosoftDocs/sdk-api/blob/656e9656bfaef34dd29cd7ee9e1146aec17e924c/sdk-api-src/content/wintrust/nf-wintrust-winverifytrust.md)。

### 2.2 结构文档与样例确实存在输出修改

WSS 文档规定 flag 1 使用 dwIndex；flag 2 才要求返回 secondary count。其 `dwVerifiedSigIndex` 描述是：“The index used for verification. This member is set on return from Wintrust.” See also 指向 WinVerifyTrust，但没有给出 FILE/CAT、Ex/non-Ex、特定 flag 组合的逐项成功覆盖合同。不能用 `[in]` 标注否定结构内输出，也不能由 See also 推断 Ex 明确不支持。[WSS 文档](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_signature_settings)。

WINTRUST_DATA 文档要求 VERIFY 接收 state handle，并对每次 VERIFY 用 CLOSE 释放；`pSignatureSettings` 指向 WSS。该文档说明有可修改的状态字段，并不保证所有字段在所有 subject 路径都更新。它列出 cache-only flag 用于防止证书网络获取；研究不建议改变现有 cache/revocation/provider flags。[WINTRUST_DATA](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_data)。

版本边界必须分开：两个入口页面的最低平台是 XP/Server 2003；WSS 页面列的是 Windows 8/Server 2012；WINTRUST_DATA 的 `pSignatureSettings` 又注明 Windows 7+KB3033929/Windows 8/Server 2012 的支持开始。固定 SDK 在 NTDDI_WIN8 条件下声明该成员。头文件版本 **10.0.26100.0 不是本次实际 OS/provider DLL 版本证明**；API 存在也不证明实际运行分支支持完整 WSS 返回合同。[两个入口](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrustex#requirements)、[WSS Requirements](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_signature_settings#requirements)、[DATA member 支持](https://github.com/MicrosoftDocs/sdk-api/blob/656e9656bfaef34dd29cd7ee9e1146aec17e924c/sdk-api-src/content/wintrust/ns-wintrust-wintrust_data.md)、SDK 161 行（`C:/Program Files (x86)/Windows Kits/10/Include/10.0.26100.0/um/WinTrust.h:161`）。

### 2.3 固定 Microsoft CodeSigning 样例的价值与限制

已完整读取 10,414 B 的官方样例。FILE 分支使用借入 handle、generic V2、WSS index 0、**flag 1|2**，并调用 WinVerifyTrust；成功后读取 secondary count。后续轮次先 CLOSE，再重置 state/action，注释提示 dwStateAction 可能被前次调用改变；cleanup 也用同 API/action/data CLOSE。[固定样例 40–139 行](https://github.com/microsoft/Windows-classic-samples/blob/434f6002bdf9cf9829406c3ff2b33387982d6168/Samples/Security/CodeSigning/cpp/codesigning.cpp#L40)。

三项限制：settings 零初始化，未设置非零 sentinel，也完全未读取或断言 dwVerifiedSigIndex；样例的 HWND、revocation、provider flags、secondary-count flag 与现 adapter 不同；名为 VerifyCatalogSignature 的分支实际只 hash/枚举 catalog，没有 WinVerifyTrust 或 WSS CAT 验证。**源码样例不是当前 OS 的正向运行实证，更不是 CAT returned-index 正实证。**[固定样例 47、69、83、148–277 行](https://github.com/microsoft/Windows-classic-samples/blob/434f6002bdf9cf9829406c3ff2b33387982d6168/Samples/Security/CodeSigning/cpp/codesigning.cpp#L148)。

## 3. 保留 UNKNOWN，不把文档升级为资格

| 问题 | 本次状态 |
| --- | --- |
| Ex 与非 Ex 的实际内部实现、复制深度、alias 关系 | UNKNOWN；没有读取 DLL 内部实现，也没有取得一方实现承诺 |
| 当前 OS / Wintrust / provider 精确运行版本及分支支持 | UNKNOWN；本次只 pin 固定 SDK 和公共源码，不查询或执行 native |
| ctypes ABI 实际调用与设置接收是否完全正确 | UNKNOWN；源码有 size/offset/签名守卫，不等于当前 provider 的运行接收证明 |
| 同 flag=1 / NULL policy / sentinel 下，Ex 与非 Ex 输出区别 | UNKNOWN；尚无只换符号的受控观测 |
| 原 settings 与返回 pSignatureSettings 的指针身份、修改目标 | UNKNOWN；现输出只记原 settings.index；不能自行添加指针跟踪或新 alias 合同 |
| FILE、CAT 各自 WSS signature selection 与 selected signer 绑定 | UNKNOWN；trust=0/state=true/close=0 不补足 index/选择合同 |
| 是否必须启用 flag 2、默认 index=0 能否作接受依据 | 未证实；样例不能证明前者必需，也不能证明零初始化就是写回；本次不修改 |
| 单符号切换能否修复 FILE，或进一步修复 CAT | UNKNOWN；即使未来 FILE 对照正向，也不能自动升级 CAT/签名者/私密/客户端资格 |

以上 UNKNOWN 来自第 1–2 节已列证据的覆盖边界，不是发现新的 OS 缺陷。ROOT 原 CAT 记录已经明确保留 catalog/WSS support、ABI semantics、actual signature selection 为研究问题。CAT 原失败记录（`C:/DevelopFile/magic-market-data-rs/.agent-state/root-fixed-ssh-state-index-failed-diagnostic-result-20261011-v1.json`）。

## 4. 若 ROOT 决定继续：精确最小前置（本次未实施）

建议只有一项先后续决策的 **SOURCE_ONLY** 候选：固定 FILE v2 的 WinVerifyTrustEx→WinVerifyTrust 单变量对照。它检验“入口符号是否影响这一次输出”，不检验所有 flags、FFI 或 CAT 的完整合同。推荐理由仅是共同入口、官方 WSS 使用方式与较小机械 delta；它不是 API 根因证明。FILE 原决策（`C:/DevelopFile/magic-market-data-rs/.agent-state/root-file-wss-v2-one-public-native-decision-20261011-v1.json`）、[WSS](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_signature_settings)、[CodeSigning](https://github.com/microsoft/Windows-classic-samples/blob/434f6002bdf9cf9829406c3ff2b33387982d6168/Samples/Security/CodeSigning/cpp/codesigning.cpp#L83)。

1. ROOT 另行给出仅 SOURCE_ONLY 的准备范围，保留旧 v2 与其负例；本研究不创建候选、actual inputs、输出、GO 决策或激活目录。原一次 native 授权有 further_retry_go=false，不能继承为重试授权。原 ROOT 决策（`C:/DevelopFile/magic-market-data-rs/.agent-state/root-file-wss-v2-one-public-native-decision-20261011-v1.json`）。
2. 审查者核对 adapter 的 **四个 API 属性引用**（restype、argtypes、VERIFY、CLOSE）一致更换到非 Ex 符号；不混用 VERIFY/CLOSE API，不顺便变更 pointer/结构、signed32/==0、HWND、flag、policy、subject/handle 或 sentinel。LPVOID 与 typed pointer 的声明差别仍需源码 FFI 解释，不能以“alias 等价”省略。固定 adapter（`C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/file_adapter.prepare.py`）、SDK 声明（`C:/Program Files (x86)/Windows Kits/10/Include/10.0.26100.0/um/WinTrust.h:462`）。
3. 新候选需要机械 delta 正逆、完整来源 pins、保留 guards/预算/allowlists/strict21/六 GO=false/default NONE 的独立 SOURCE_ONLY 审查；checker 若有精确符号断言，只做对应静态期待更新，不能增加 collector、放宽 failure 或丢弃真实错误观测。既有 28/30 秒、19 inputs/43 peak/96 cap、同 action/data finally CLOSE、3 custody/17 reads/1 MiB 的限制不因符号对照而扩大。原 ROOT 决策（`C:/DevelopFile/magic-market-data-rs/.agent-state/root-file-wss-v2-one-public-native-decision-20261011-v1.json`）。
4. 本研究与 SOURCE_ONLY 通过均不授权 native。只有 ROOT 另行批准、来源封存与固定公共身份 fresh 验证后，才可能进行新的精确一次公共 FILE 观测；任何自然 exit/三个原始输出/关闭与 custody 结果都须保留，`returned_index=FFFFFFFF` 仍是 INDEX_MISMATCH，不接受默认零、自填零或忽略 index。原 ROOT 决策 acceptance / interpretation（`C:/DevelopFile/magic-market-data-rs/.agent-state/root-file-wss-v2-one-public-native-decision-20261011-v1.json`）。
5. 无论未来 FILE 返回什么，都不授予 CAT selection、Microsoft signer、私密/SSH/客户端/服务/发布资格；原 CAT 失败不能反向抹除。新的正/负对照也仅对应固定 subject 与固定参数，不是 OS 通用合同。CAT 原失败记录（`C:/DevelopFile/magic-market-data-rs/.agent-state/root-fixed-ssh-state-index-failed-diagnostic-result-20261011-v1.json`）。

上述是条件性建议，不是新 packet 的准备或 GO。ROOT canonical V10 session 27281 仍运行这一事实来自本次 ROOT 消息；本研究不检查、不干预该进程，正式源码冻结边界保持。

## 5. 完整来源 pins 与实际读取范围

### 5.1 本地公共证据（原始字节 SHA256）

下表八项是本研究直接读取/哈希的来源，不是对整个活动叶 exact-set 的重新资格审查。FILE 叶已激活；未执行 SOURCE_ONLY absence checker，也未运行任何 prepared runner/adapter。

| 来源（绝对路径） | bytes | SHA256 | 阅读范围 |
| --- | ---: | --- | --- |
| C:/DevelopFile/magic-market-data-rs/.agent-state/root-file-wss-v2-one-public-native-decision-20261011-v1.json | 3433 | 094d6c3f5337ac44b661a151d54d6619a71bd119f135f4fcf2b5c1e2d437e589 | 全文 |
| C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/reservation.json | 327 | e8e31b5e389526dfe33fccf88a1516251506e905f9a3f70db19485282a2151dc | 全文 |
| C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/journal.jsonl | 120 | 222b48450e89e1f220a8d0aa26ef4ee880f109915d5ab2ea9d06a6fe1dc7a80e | 全三行 |
| C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/receipt.json | 1232 | c34d58d800246610349d6e24693e0c22cbe915812ccf8ede9dc34d3b476ad20b | 全文 |
| C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-app-python-file-wss-control-source-20261011-v2/file_adapter.prepare.py | 9411 | f019a5f4e64289361126ef82d5c2bfc202a513414dcbea095db93c5a1f5f8cfa | 全文；只读，不导入 |
| C:/DevelopFile/magic-market-data-rs/.agent-state/windows-fixed-ssh-catalog-state-index-diagnostic-source-20261011-v1/receipt.json | 1110 | ba5a7dade1b925e9eb0ade63c9a6d5d74a9874414a9c673a24ec4d1ad69d6b50 | 全文 |
| C:/DevelopFile/magic-market-data-rs/.agent-state/root-fixed-ssh-state-index-failed-diagnostic-result-20261011-v1.json | 2942 | da477737bcb5775e490ccc4c384a1326fd8a53ce1b12f260aa1235c5cd75f425 | 全文 |
| C:/Program Files (x86)/Windows Kits/10/Include/10.0.26100.0/um/WinTrust.h | 73549 | 340a90182fc0fddaac188a2921bb49dd3a5486d968831764dbde81bb0beedbeb | 全文件哈希；实际阅读 88–209、450–470 行，非全文审查 |

### 5.2 第一方在线来源（固定 commit、HTTP 200 原始响应字节）

只用 Microsoft/MicrosoftDocs 仓库与 Microsoft Learn。GitHub 公共 GET 先解析 commit，再取固定 raw 文件；未克隆、构建、执行或落盘这些源文件。SHA 是 RawContentStream 的原始字节，不是网页抽取文本或重新编码后的字节。

| 固定原始来源 URL | bytes | SHA256 |
| --- | ---: | --- |
| https://raw.githubusercontent.com/microsoft/Windows-classic-samples/434f6002bdf9cf9829406c3ff2b33387982d6168/Samples/Security/CodeSigning/cpp/codesigning.cpp | 10414 | e3189d1d2228c6c3b0aaca4761241e50bf28d0e04176f84f5f8ad318ba3edbb6 |
| https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/656e9656bfaef34dd29cd7ee9e1146aec17e924c/sdk-api-src/content/wintrust/nf-wintrust-winverifytrustex.md | 10396 | 566e89d993194c3d0047a627b35ca9f53474bed8536890484cc2425dc2744062 |
| https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/656e9656bfaef34dd29cd7ee9e1146aec17e924c/sdk-api-src/content/wintrust/nf-wintrust-winverifytrust.md | 11192 | 817f946f64ed728f88cbcd15e2bf9d46ceb1b18415f56fbd227577fd66616a0b |
| https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/656e9656bfaef34dd29cd7ee9e1146aec17e924c/sdk-api-src/content/wintrust/ns-wintrust-wintrust_signature_settings.md | 3741 | 1b3c95b341767566d3e858173d51361b7018d44cf49a6abcd1dd235c5a5ea1a6 |
| https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/656e9656bfaef34dd29cd7ee9e1146aec17e924c/sdk-api-src/content/wintrust/ns-wintrust-wintrust_data.md | 16609 | 853f13b4e850d7f6f46c483a751893d711c5bcb5208e7987dcfe601c8ed25511 |

五份固定源文件均全文读取。解析得到 classic-samples main=434f6002bdf9cf9829406c3ff2b33387982d6168（committer date 2026-09-03T00:00:00Z），sdk-api docs=656e9656bfaef34dd29cd7ee9e1146aec17e924c（2026-10-09T20:25:44Z）；此处时间是仓库 commit 元数据，不是样例实际试验时间。[classic commit](https://github.com/microsoft/Windows-classic-samples/commit/434f6002bdf9cf9829406c3ff2b33387982d6168)、[docs commit](https://github.com/MicrosoftDocs/sdk-api/commit/656e9656bfaef34dd29cd7ee9e1146aec17e924c)。

Learn 页面是阅读时的在线呈现，不声称其 HTML 与上述 raw Markdown 字节相同：两个入口页显示 last updated 2021-10-13，WSS 页显示 2024-02-22；固定 raw Markdown 的 ms.date=12/05/2018。保留呈现日期与源元数据差别，不用它们推断 runtime 版本。[Ex](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrustex)、[WinVerifyTrust](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrust)、[WSS](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_signature_settings)。

## 6. 过程与核验边界

使用 autonomous-long-task 的常规只读推进边界、research 的后台第一方来源与单 Markdown 交付要求、verification-before-completion 的 fresh 证据优先规则。SDK 是已知精确外部路径，packet 是已指定公共证据，不进行 CodeGraph 索引重建或代码定位扩扫。只读命令输出：81491f（SDK 关键全文区间）、589625（两个 commit）、856fd6（样例全文/原始字节 pin）、bc6398（三份 API/WSS 原文）、010916（DATA 原文）、54de82（FILE/decision/adapter/SDK fresh pins 与全文）、25d826（CAT 两项 fresh pins 与全文）。

保留一次读取命令构造失败：9ae767 把 PowerShell `+` 和逗号混合为一个拼接路径，出现 Get-Item/Get-FileHash 非终止错误，虽 shell exit 0 但不算来源验证；只纠正读取命令为 literal-path 数组并加 Stop，54de82 才完成成功读取。没有更改任何被审查来源或重新执行诊断。

没有调用 native/WinTrust/ctypes/SSH/Cargo、没有导入 prepared 源码、没有读取私密/凭据、没有创建输入或输出 packet、没有调整接受条件。本 Markdown 的全文回读、最终来源 fresh pin 与报告自身 bytes/SHA 由交接命令提供；不另外创建 REFS/SEAL 文件。交接后停止写入。
