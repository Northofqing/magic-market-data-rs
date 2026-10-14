# MSYS native docs-checker fault — 2026-10-07

## Finding and limits

The original docs checker has captured native write access violations inside the installed MSYS2 DLL, at RVA `0x16e719`. The coordinating agent's separately retained PE and register captures establish the direct failure mechanism: the `sigdelayed` XSAVE branch subtracts a full-width RBX value with a nonzero upper half, then the write at `RSP + 0x24` faults. This researcher independently read those receipts. Those fault-time captures alone do not identify where the upper bits originated. A subsequent standalone CPUID discriminator, recorded below by the coordinating agent, reproduces preservation of seeded upper bits on this host without MSYS.

A particularly relevant upstream report describes stale upper `RBX` bits after a virtualized `CPUID` causing a multi-gigabyte stack adjustment in the same source routine. Its proposed defensive change is an **open draft**, not a merged or released repair. The local standalone discriminator now establishes abnormal upper-bit preservation; it does not attribute this behavior to TwinCAT, a hypervisor or any specific driver. [MSYS2 issue #363](https://github.com/msys2/msys2-runtime/issues/363), [draft PR #378](https://github.com/msys2/msys2-runtime/pull/378)

## Exact local evidence

The worktree inspected is `C:/Users/13687/.codex/worktrees/grpc-coverage-20261002/magic-market-data-rs`, at HEAD `7174f09f0b34d082bc584180ba4260d53f379d6c`. The original command retained by the capture is `C:/Program Files/Git/bin/bash.exe -x tools/docs/check_links.sh`.

The earlier [capture result](../../../target/docs-checker-late-exception-20261007.1/RESULT.json) records eight baseline and eight late-debug observations. Baseline has one failed command; late-debug has two. The four AV events are first-chance and second-chance pairs for those **two** processes, not four independent failures. Debugger timing differs from baseline; these small fixed counts do not establish a population crash rate or prove that attachment caused the failure.

| Capture | Owned main PID / TID | Fault RVA / write address | Captured RSP / RBP |
| --- | --- | --- | --- |
| [late-debug 5](../../../target/docs-checker-late-exception-20261007.1/late-debug-5/OBSERVATION.json) | 58688 / 58968 | `0x16e719` / `0xfffffffdffffaab4` | `0xfffffffdffffaa90` / `0x7ffffaec0` |
| [late-debug 6](../../../target/docs-checker-late-exception-20261007.1/late-debug-6/OBSERVATION.json) | 59304 / 35820 | `0x16e719` / `0x6ffffb6b4` | `0x6ffffb690` / `0x7ffffba90` |

Both pairs have code `0xc0000005`, access mode 1 (write), module `C:/Program Files/Git/usr/bin/msys-2.0.dll`, and final process exit code 3221225477. The records explicitly label their stack candidates as filtered code addresses rather than unwound call frames; their empty arrays are not a valid call stack.

Fresh read-only file metadata and hashes are retained in the new [source receipt](../../../target/msys-native-fault-research-20261007.1/SOURCE-RECEIPT.json), captured at `2026-10-07T01:58:16.6018666Z`.

| File | Identity independently read here | SHA-256 |
| --- | --- | --- |
| `C:/Program Files/Git/usr/bin/msys-2.0.dll` | File/ProductVersion `3.6.9-b4195d69133078c498a1bf811c4fb0c61fc3c8af`; 3,368,543 bytes | `2ea49553e4c03055dcf1c4a2bef54668081a07663fba283f4b34cf70f2157191` |
| `C:/Program Files/Git/usr/bin/bash.exe` | 2,456,832 bytes; PE version fields absent. Bash `5.3.15(1)` is supplied by the earlier investigation, not newly executed here. | `acf4ecb52e601f7b4a37db51b07650b5d0315eafd010590e98079fa026da4b7b` |
| `C:/Program Files/Git/bin/bash.exe` | Launcher File/ProductVersion `2.55.0.windows.3` | `a80d0cc4ed45f99c6073e8504bcdd9935c553afd9460627cbbb4bec103e7ef60` |
| [original checker](../../../tools/docs/check_links.sh) | 610 bytes; unchanged | `5cbc626ce1fbfc2676c8533cd914358984b2ec49538d2462c800b846d1f496d2` |

The earlier minimal-pattern and synchronous/async producer observations supplied to this researcher remain separate experiment evidence: live synchronous fresh producer 10 passes versus async producer 4 failures in 10 runs. The comparison changes both execution timing and how input is materialized. It does not identify a runtime root cause or prove a checker fix.

### Separately captured native mechanism

The coordinating agent subsequently retained a [12-run register capture](../../../target/docs-checker-register-capture-20261007.1/RESULT.json) with four failed processes and eight paired first/second-chance AV events. SHA-256 of that result is `dc40a311dbdf085cb803bc0afa01a7c3a564f69e4d09ea6f1a494104784bf29f`. The four failures have the following first-chance values; each matching second-chance record retains the same values.

| Ordinal | RCX | RBX used for subtraction | Captured RSP |
| --- | --- | --- | --- |
| 5 | `0x100000340` | `0x100000388` | `0x6ffffb690` |
| 6 | `0xa00000340` | `0xa00000388` | `0xfffffffdffffbdd0` |
| 9 | `0x100000340` | `0x100000388` | `0x6ffffb690` |
| 10 | `0xa00000340` | `0xa00000388` | `0xfffffffdffffc050` |

All eight records satisfy the local prologue relation `RSP = aligned prologue stack - 13 * 8 - RBX`; the fault write address is `RSP + 0x24`. The low allocation portion is `0x388`, while the high 32 bits are 1 or 10. That establishes an oversized full-width subtraction at failure, not why those bits existed.

The separately retained [PE instruction correspondence](../../../target/docs-checker-register-capture-20261007.1/PE-INSTRUCTION-CORRESPONDENCE.json), SHA-256 `59cb63c7aefead568d8295dd29e7a1722aa30d8f63c51597a3c5bd58d83b659d`, binds 15 asserted opcode segments to the same DLL hash. Its corrected sequence places CPUID at RVA `0x16e705`, copying RBX to RCX at `0x16e707`, subtraction at `0x16e716`, and the faulting `89 5c 24 24` write at `0x16e719`. The coordinating agent reported that an initial one-byte RVA displacement failed an assertion before correcting the addresses; that initial failure remains separate evidence, not a passing check. The correspondence identifies the XSAVE branch, before execution reaches XSAVE itself.

## Pinned official source

### Subsequent standalone CPUID discriminator

After the bounded source-research assignment completed, the coordinating agent
compiled a [standalone Rust/assembly probe](../../../target/docs-checker-register-capture-20261007.1/cpuid.rs)
with the existing installed compiler and captured its [original stdout](../../../target/docs-checker-register-capture-20261007.1/cpuid.stdout.log),
[stderr](../../../target/docs-checker-register-capture-20261007.1/cpuid.stderr.log)
and [receipt](../../../target/docs-checker-register-capture-20261007.1/CPUID-OBSERVATION.json).
The compiler invocation and executable/source hashes are in that receipt;
compilation output `9eb9da` and probe output `2e7242` both have native exit 0.

The probe sets RBX to four fixed seeds, executes CPUID at subleaf zero, then
records the full 64-bit result immediately without a 32-bit wrapper truncation.
It restores the ABI's nonvolatile RBX and touches only its own output buffer.
For each of leaves 0, 1 and 13, all 12 nonzero-seed observations preserve the
seed's upper 32 bits; the four zero-seed observations have zero upper bits.
There are 48 observations in total. Leaf 13 returns low RBX `0x340`; with seed
`0x100000000` or `0xa00000000` the full result matches the size copied into RCX
in the actual Bash failures. This establishes the host's CPUID upper-bit
preservation independently of MSYS and the concrete oversized-allocation
failure mechanism; it is not a statistical crash-rate estimate or a runtime fix.

The installed MSYS DLL hash is unchanged before/after the probe. No DLL patch,
injection, driver setting, system PATH, WER, Provider, RPC or service control is
involved. A separate read-only machine query reports a Parallels virtual-platform
model and no result for the exact `TcRTime` system-driver name; that does not
prove which component causes the CPUID behavior or that no other such driver
exists. Vendor correction or a reviewed guard still needs controlled validation
and separate environment authority.

The version suffix resolves to Git for Windows runtime commit [b4195d69133078c498a1bf811c4fb0c61fc3c8af](https://github.com/git-for-windows/msys2-runtime/commit/b4195d69133078c498a1bf811c4fb0c61fc3c8af), a merge dated `2026-06-06T12:48:01Z`. Its message identifies a synchronization with MSYS2 3.6.9 for a pseudoconsole teardown fix; that purpose does not establish the cause of the captured AV.

The exact [gendef source](https://github.com/git-for-windows/msys2-runtime/blob/b4195d69133078c498a1bf811c4fb0c61fc3c8af/winsup/cygwin/scripts/gendef) was independently decoded from the public GitHub contents API. Canonical decoded-byte receipt: Git blob `6328fe2fbddbb01c448613b6938dd6a806985fa0`, 11,360 bytes, SHA-256 `74f502440493f2406435006bce46674adcfda59b269f0da98586a7b87e0127fb`. Line numbers below refer to that decoded file; rendered web text strips some blank lines.

In the x64 `sigdelayed` generation, lines 232–240 query XSAVE size, copy full-width `RBX` into `RCX`, round the allocation size up to a 64-byte multiple, add overhead, subtract full-width `RBX` from `RSP`, then execute `movl %ebx,0x24(%rsp)`. The FX-save fallback also has an `RSP + 0x24` write. Displacement matching alone is ambiguous; the separate local PE correspondence above resolves this capture to the XSAVE branch. [Pinned source, lines 221–240](https://github.com/git-for-windows/msys2-runtime/blob/b4195d69133078c498a1bf811c4fb0c61fc3c8af/winsup/cygwin/scripts/gendef#L221-L240)

The known alignment repair from [MSYS2 commit aa532e7bbf59a3ec1e29e13c9e8515d545cb2e16](https://github.com/msys2/msys2-runtime/commit/aa532e7bbf59a3ec1e29e13c9e8515d545cb2e16) is **already present** at lines 236–237. [Issue #295](https://github.com/msys2/msys2-runtime/issues/295) reports an `xsave64` alignment fault under Wine with PKU enabled; that is a different observed fault condition from the present `RSP + 0x24` write. Reapplying the alignment fix is not an established remedy here.

Git for Windows runtime `main` resolved to [81d9bd3d1c3680aa611ff2e5d1950787ffc95c6a](https://github.com/git-for-windows/msys2-runtime/commit/81d9bd3d1c3680aa611ff2e5d1950787ffc95c6a), dated September 28. The contents API returns the **same gendef Git blob and byte size** at that commit. A [pinned comparison](https://github.com/git-for-windows/msys2-runtime/compare/b4195d69133078c498a1bf811c4fb0c61fc3c8af...81d9bd3d1c3680aa611ff2e5d1950787ffc95c6a) reports 171 commits ahead and no net gendef file change. This bounds the inspected source difference; it does not certify any downloaded release binary or rule out changes in other files.

## Relevant upstream evidence and status

| Primary evidence | What it supplies | Applicability limit |
| --- | --- | --- |
| [MSYS2 issue #363](https://github.com/msys2/msys2-runtime/issues/363), opened September 11 | The reporter reproduces shell-substitution AVs with Git 2.55.0.windows.5 / MSYS 3.6.10 under TwinCAT RUN. A separate assembly probe reports stale upper RBX after `CPUID(0xD,0)`; CONFIG mode clears the high half. | Reporter evidence for another host, not an observation of this host. It motivates recording the full RBX value and preceding instructions. |
| [Maintainer response on #363](https://github.com/msys2/msys2-runtime/issues/363#issuecomment-6012177905), October 6 | The maintainer considers the external CPUID implementation suspect, requests proper patch/testing context and favors the Cygwin submission route for inherited code. | This is not upstream confirmation of a universal MSYS bug or a released fix. |
| [Draft PR #378](https://github.com/msys2/msys2-runtime/pull/378), opened October 6; [proposed file change](https://github.com/msys2/msys2-runtime/pull/378/files) | Head `34e9927ef854052e4fef9bc147c93462ba9d735d` proposes one added instruction, `movl %ebx,%ebx`, after the XSAVE CPUID query and before copying RBX. The author reports testing a patched DLL in RUN/CONFIG modes. | At the captured API time: open, draft, `merged=false`, base `msys2-3.6.10` at `3ea87a506e64e841dc0b8ee50ed61999c8fed26c`. This guard is absent from the inspected installed revision and inspected Git for Windows main. No patched DLL was acquired or tested here. |
| [MSYS2 issue #362](https://github.com/msys2/msys2-runtime/issues/362) | Reports intermittent parallel-build hangs with 3.6.10 and a reporter rollback to 3.6.9. | A hang in a different workload is not a matching AV signature and does not justify a rollback here. |

The latest Git for Windows release visible during this research is [2.56.0.windows.2](https://github.com/git-for-windows/git/releases/tag/v2.56.0.windows.2), released October 5. Its listed changes include UCRT migration follow-up work; the release page does not identify PR #378 as a fix. Merely choosing a newer version number is not evidence that this native fault is repaired.

## Minimum remediation options and authority boundaries

1. **Repository-only candidate:** materialize and check the producer result synchronously, then let the original validation loop read that completed input. This can avoid the observed async boundary while preserving link validation and explicit producer failure handling. It must be evaluated as a checker workaround with bounded original-command controls and broken-link/producer-failure cases; it must not be described as a repaired MSYS runtime. No checker edit is made by this research.
2. **Runtime comparison:** if runtime remediation is selected, use a separately approved, isolated complete Git distribution with an exact version, package hash, runtime hash and source provenance. Keep the original checker and host configuration fixed. The currently inspected main has the same candidate routine; neither an upgrade nor downgrade is proven useful. Installing, extracting for execution, replacing or upgrading the runtime is outside this read-only assignment and requires environment authority.
3. **External-emulation candidate:** only if native evidence establishes abnormal CPUID register results, investigate the relevant vendor's supported correction. Changing driver/virtualization configuration, rebooting or changing TwinCAT modes requires explicit environment/operational approval. The draft zero-extension patch remains an experimental option requiring source review, a reproducible build and separate execution authority.
4. **Evidence handoff:** retain a minimal native reproducer and exact module/instruction/register evidence for upstream review if needed. Posting an issue, PR or message requires explicit communication authorization.

Local instruction and fault-time register captures establish the direct oversized-allocation mechanism, and the later standalone probe reproduces abnormal CPUID upper-bit preservation. Attribution to a driver, hypervisor or TwinCAT, and effectiveness of any remedy, still require separate evidence and applicable authority. The actual MSYS CPUID instruction was not instrumented before/after, and no runtime was patched.

## Work performed and verification scope

The background source-research assignment used public official repositories, release pages and issue/PR API records, plus read-only local file version/hash inspection. It did not execute a runtime, attach a debugger, install or replace tools, change PATH/configuration/environment/WER, call Provider/RPC/service interfaces, or alter an old capture. Its writes were this note and the new source receipt. The coordinating agent's later standalone probe and note update are explicitly separate work above. Receipt JSON parse, exact local hashes, cited local target existence and diff whitespace are checked before handoff. This is not a passing release gate or a claim that the runtime fault is fixed.
