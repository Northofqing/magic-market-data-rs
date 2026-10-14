# PowerShell Authenticode cmdlet 与严格 signer/catalog 契约：第一方来源比较

日期：2026-10-11。性质：后台 PRIMARY 来源研究，SOURCE_ONLY；不是修复、第 4 次签名对照、接受条件变更、native 资格或 GO。仅新增本 Markdown；不调用 cmdlet/WinTrust，不打开实际被验 SSH/catalog/Python 文件，不读取私密、环境值或证书存储，不改旧证据。

## 结论

**不能把 `Get-AuthenticodeSignature` 认作当前严格 Microsoft leaf / 精确 catalog / 同一选定链 / revocation / weak-digest 合同的等价替代。** 公开的固定版本源码能说明它如何取得签名摘要，却不能提供上述全部绑定证据。相同的 held path/hash/link/custody 守卫解决的是文件身份边界；它们不自动解决 verifier 的签名、catalog 和链选择边界。下文区分源码明确差异、文档合同与 UNKNOWN，不建议自动 fallback。

ROOT 本次通知的既有结果为 ExCAT、ExFILE、nonExFILE 均 trust=0、CLOSE=0、state=true，但 index=FFFFFFFF，因此严格选择合同失败。这是 ROOT 已关闭观测的上下文，本研究没有重新执行或独立重审全部 native receipts。该现象不证明 cmdlet 会选择同一签名，也不授权把 sentinel 当作 0、忽略 index 或接受摘要替代。较早的 [WSS 研究记录](C:/DevelopFile/magic-market-data-rs/docs/integrations/research/2026-10-11-winverifytrustex-wss-writeback-primary.md) 保持历史原样，其当时待决单符号建议不构成本次重试建议。

## 1. 固定版本、完整阅读范围和版本限制

研究对象选用公开 **PowerShell v7.5.3**，对应官方 release commit **b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac**；这是一个明确可复查的实现样本，不声称它是最新版本、当前安装版本或 Windows PowerShell 5.1 的实现。[官方 release](https://github.com/PowerShell/PowerShell/releases/tag/v7.5.3)、[固定 commit](https://github.com/PowerShell/PowerShell/commit/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac)。

下表逐项列出官方 raw 源的完整实际阅读范围；表内为本次网页抽取的行范围（工具从 0 开始），五文件全文、另一个文件只读完整相关结构。`Signature` 类的实际文件名是 `MshSignature.cs`，不是猜测的 `Signature.cs`。固定链接指向上述 commit；未克隆、构建、安装或加载源码。

| 固定源 | 实际阅读范围 |
| --- | --- |
| [Authenticode.cs](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/System.Management.Automation/security/Authenticode.cs) | 全部 0–553；核对 Get 路径，不把 SignFile 的选项混入 Get |
| [SignatureCommands.cs](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/Microsoft.PowerShell.Security/security/SignatureCommands.cs) | 全部 0–580；包括公共参数、路径解析及 PerformAction |
| [MshSignature.cs](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/System.Management.Automation/security/MshSignature.cs) | 全部 0–361；包括对象字段与状态映射 |
| [Win32Native/WinTrust.cs](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/System.Management.Automation/security/Win32Native/WinTrust.cs) | 全部 0–405；结构与 WinTrust/helper 声明 |
| [System.Management.Automation.csproj](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/System.Management.Automation/System.Management.Automation.csproj) | 全部 0–95；Microsoft.Security.Extensions 依赖固定为 1.4.0 |
| [nativeMethods.cs](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/System.Management.Automation/security/nativeMethods.cs#L709) | 完整相关结构 708–741；没有宣称全文审查该 1386 行文件 |

Microsoft.Security.Extensions **1.4.0** 的官方包页说明其用途为文件签名信息，列 Microsoft 所有者；project 链接指向 cmdlet 文档而非可核对的实现源码。本次未找到可完整核验该版本内部验证策略的第一方源码。没有下载/反编译/执行该包；它的 native flags、catalog 选择、链策略及清理内部实现仍为 **UNKNOWN**。官方包身份不是这些具体语义的证明。[1.4.0 包元数据](https://www.nuget.org/packages/Microsoft.Security.Extensions/1.4.0)。

## 2. 公开 cmdlet 实际返回什么

文档说明 Windows catalog 与 embedded signature 同时存在时使用 catalog signature；公开入口提供路径或内容参数，`LiteralPath` 不展开 wildcard。Get 没有 caller handle、固定 catalog、hCatAdmin、member hash/tag、签名索引或信任策略参数。字节内容入口也不是一个固定 PE/catalog-member 验证接口。[官方 Get 文档](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/get-authenticodesignature?view=powershell-7.5)。

固定 cmdlet 源将路径交给 `SignatureHelper.GetSignature(path, null)`；内容交给同 helper。文件存在/PS provider 路径解析不等于本项目的 custody 守卫。该文件里的 `HashAlgorithm` 属于 **Set**，不能当作 Get 可指定 SHA256 的依据。[固定 SignatureCommands.cs](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/Microsoft.PowerShell.Security/security/SignatureCommands.cs#L210)。

两条内部路线的源码事实：路径先经 `File.OpenRead` 调用 `FileSignatureInfo.GetFromFileStream`，其 SignedAndTrusted 状态映射为 0；库异常可使后续调用跳过该路线。结果 null 或非 Valid 时回退 WinVerifyTrust。fallback 使用 generic V2、FILE（内容为 PowerShell SIP BLOB）、UI_NONE、VERIFY；未设 revocation/provider flags，二者为 0；FILE 不借入调用者 handle。它从 provider signer 0 / cert 0 复制证书，随后 CLOSE；CLOSE 非零只追踪，不改变已构造结果，且此块不是 finally。[固定 Authenticode.cs，256–543](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/System.Management.Automation/security/Authenticode.cs#L256)。

公开 `Signature` 是摘要对象：Path、Status/StatusMessage、SignerCertificate、TimeStamperCertificate、SignatureType、IsOSBinary。它不暴露精确 catalog 路径、同一 hCatAdmin/hash、verified-signature index、provider state/borrowed chain 或 checked CLOSE 结果；0 映射为 Valid。不能由 Valid 或证书 subject 字符串反推本项目全部合同已完成。[固定 MshSignature.cs](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/System.Management.Automation/security/MshSignature.cs)。

固定 interop 的 WINTRUST_DATA 截至 dwUIContext，不包含 pSignatureSettings；相关 provider-signer 结构虽有链上下文字段，却未作为公共 Signature 证据输出。这是该固定实现的限制，不是本机 ABI 或 provider 的运行证明。[固定 WinTrust.cs](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/System.Management.Automation/security/Win32Native/WinTrust.cs#L150)、[相关 provider 结构](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/System.Management.Automation/security/nativeMethods.cs#L709)。

## 3. 官方 Windows 合同与当前严格边界的差别

以下比较采用 ROOT 已说明的当前要求：primary signature 0、请求 0x3040/whole-chain、同一选定 provider chain 的 Microsoft-root policy7、同 SHA256 hCatAdmin member hash 与固定 catalog 身份、每次 attempted VERIFY 的 finally/checked CLOSE。它不是本次重新批准的要求清单。

| 边界 | 第一方事实与结论 |
| --- | --- |
| exact signature selection | WSS 的 dwIndex 配合 WSS_VERIFY_SPECIFIC；dwVerifiedSigIndex 是返回的实际索引。所读文档没有赋予 FFFFFFFF 成功含义。摘要不能补出请求/返回绑定；provider signer ordinal 0 也不能自行等同于一次具体 WSS 请求的输出。[WSS](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_signature_settings)、[signer helper](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-wthelpergetprovsignerfromchain) |
| same selected chain | provider signer 携带自己的 pChainContext；从它取得证书与另建一条 X509Chain 是不同操作。**推论**：事后重建链可能成为替代链，不能仅凭同叶证书证明就是原次验证链；本研究不建议该替代。[CRYPT_PROVIDER_SGNR](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-crypt_provider_sgnr)、[证书 helper](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-wthelpergetprovcertfromchain) |
| Microsoft 身份 | policy7 检查第一条 simple chain 的末元素 Microsoft root 公钥，不是 leaf Subject 文本；它自身不执行其它 policy 检查，也不做 revocation。调用 TRUE 与 policy dwError=0 要分开。IsOSBinary/Publisher/Valid 不替代这个组合。[CertVerifyCertificateChainPolicy](https://learn.microsoft.com/en-us/windows/win32/api/wincrypt/nf-wincrypt-certverifycertificatechainpolicy) |
| exact catalog/member | CATALOG_INFO 包含 catalog 路径、member 路径/tag/handle、计算 hash 与 hCatAdmin；非 SHA1 hash 的 admin 不能为 0。自动选出的 Catalog 摘要不足以证明是 ROOT 固定且已持有的那只 catalog。[CATALOG_INFO](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_catalog_info) |
| hash algorithm 的上下文 | AcquireContext2 可以明确算法；NULL 默认随系统/strong policy 而定。CalcHashFromFileHandle2 接受实际成员 handle 与该 context。外部普通全文件 SHA256 pin 与这种 catalog-member hash/上下文绑定不能自动互换。[AcquireContext2](https://learn.microsoft.com/en-us/windows/win32/api/mscat/nf-mscat-cryptcatadminacquirecontext2)、[CalcHashFromFileHandle2](https://learn.microsoft.com/en-us/windows/win32/api/mscat/nf-mscat-cryptcatadmincalchashfromfilehandle2) |
| revocation / weak digest | 0x3040 = 0x40 chain + 0x1000 cache-only + 0x2000 禁 MD2/MD4。fdwRevocationChecks=0 不等于证明禁止网络；文档要求 code-signature 无网络检索时设置 cache-only。也不能把禁 MD2/MD4 说成禁止全部弱摘要/SHA1。固定 fallback 的 flags=0 不表达这份显式策略；库路线策略 UNKNOWN。[DATA flags](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_data) |
| trust 与 lifecycle | WinVerifyTrust 仅 0 成功，不用 HRESULT 成功宏；成功只针对请求的 action/subject。每次 VERIFY 均应 CLOSE，state 成功与清理成功分开。cmdlet 摘要没有当前 strict acceptance 所需的 checked-close/finally 证据；不能以托管外壳推断它已满足。[WinVerifyTrust](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrust)、[DATA state action](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_data#members) |

### held 文件身份能解决什么，不能解决什么

如果外层确实保留已验证的原文件/祖先句柄、拒绝替换和写入、固定 volume/link/hash，并完整重验，路径重开有机会被限定为同一对象；本研究未做这一兼容试验。`File.OpenRead` 按路径新开只读 stream，使用 FileShare.Read，而 Windows FILE_INFO 原本允许借入 read handle。**推论**：share0 的持有方式可能与重开冲突，应 fail closed，不能为 cmdlet 放松 share/custody。无论是否能重开，文件身份相同仍不补足上一表的策略与选择证明。[File.OpenRead](https://learn.microsoft.com/en-us/dotnet/api/system.io.file.openread?view=net-9.0)、[FILE_INFO](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_file_info)。

`-Content` 也不是规避这一限制的方法：固定 fallback 用 PowerShell SIP BLOB，且不走 catalog-aware 文件库。把已持有 PE 的 bytes 填入内容入口不会自动等价于原 FILE/CATALOG-member 证明；本研究不提出这种新 subject 设计。[固定 Authenticode.cs，408–456](https://github.com/PowerShell/PowerShell/blob/b72c7ab1238c2d95b5c9004bca8399b8b3ca88ac/src/System.Management.Automation/security/Authenticode.cs#L408)。

## 4. UNKNOWN 与材料决策

| 问题 | 状态 |
| --- | --- |
| 当前实际 PowerShell host/module、.NET/库/native loader 的精确版本与上述 commit 对应性 | UNKNOWN；未执行版本查询、模块加载或实际 cmdlet。公开 7.5.3 不能证明本机或 5.1 等价。 |
| Microsoft.Security.Extensions 1.4.0 内部 catalog/signature selection、revocation/network/weak-digest policy、cleanup | UNKNOWN；package 元数据及调用点不足以证明。 |
| 这台 OS/provider 在固定参数下为何未写回 WSS index；Ex/nonEx 的内部分支 | UNKNOWN；新研究没有定位因果，也没有更改 sentinel 合同。 |
| 固定 catalog 和自动 catalog 是否相同、SignerCertificate 是否当前要求的那条选择 | UNKNOWN；没有实际 cmdlet/catalog/member 观测；摘要字段不足以独立重建。 |
| exact held/share/custody 下路径重开是否可行、全链/缓存可用性和耗时 | UNKNOWN；未做 subject/FS/native 试验。 |
| 可否由另一个公开版本的完整实现取得更强且同语义证据 | 待明确版本的一方源码证明；不能由本研究推断肯定或否定。 |

因此目前保留严格合同并停止重复签名试验是证据一致的状态，而不是判定标准 cmdlet 有缺陷或不安全。

若未来选择把资格改为“官方 cmdlet 返回 Valid + 某证书/OS marker + 外层身份 pin”，这将改变 catalog/signature/chain selection 与显式策略的 acceptance，而非机械复用。必须另作 **material Gate A**：明确允许哪些策略由 OS/library 默认决定、是否接受自动 catalog 或替代链、保留何种清理/无网络证据，以及哪些旧严格断言不再成立。研究本身不批准这项选择。把 cmdlet 仅作为非资格的额外摘要也仍需新的明确 scope/执行授权；本研究不创建它。

当前没有自动 fallback、sentinel 接受、弱化链/摘要/缓存政策、私密/SSH/service/client/release GO；95 门槛与正式业务源码不受本研究影响。旧 trust=0/index 失败必须保留。

## 5. 过程、来源与封存边界

依照 autonomous-long-task 的只读常规推进、research 的后台 PRIMARY 来源与一个 Markdown 交付、verification-before-completion 的实读后报告规则。在线依据仅 PowerShell 官方仓库、Microsoft Learn 与 Microsoft-owned 官方 NuGet 元数据；搜索出现的第三方博客/使用项目没有用于结论。Learn 是本次阅读时页面，不声称其 HTML 与固定仓库源码逐字一致或等于本机 OS 行为。

完整阅读证据：此前完整源读取加本次 WinTrust.cs 尾段、nativeMethods.cs 两结构与各 Windows 文档核对。一次聚合工具输出截断了 nativeMethods 段，随后单独读取相关完整结构；未把截断当全文。保留来源发现错误：猜测 Signature.cs 返回 404，改读实际 MshSignature.cs；NuGet project redirect 经工具返回 500，未据此取得库实现或进行非第一方替代。它们是资料读取问题，不是业务/native RED。

本次直接全文读取的旧 WSS Markdown：**22445 B / SHA256 fc0b123c1af950ece6269f806d374318dad44505ee961eb0a59e7501ac60f4e7**（只读核验 6fab85，exit0）。这只是历史研究引用的当前字节 pin，不重新采信其过时 pending 状态或重放其原试验。在线源码由固定 release/commit 识别，未声称网页抽取文本有原始响应 bytes/SHA pin。

没有 native/API/cmdlet/crypto 调用、prepared import、Cargo、安装、证书/registry 查询、目标/私密/环境值读取，也未创建 runner、fixture、inputs、GO 或新执行 packet。仅本新 Markdown 由 apply_patch 创建；交接前全文回读并取得自身原始字节 bytes/SHA，随后 STOP WRITING。SOURCE_ONLY 阅读结论不能升格为 ABI、custody、实际 signature selection 或发布证明。
