# MSYS CPUID remediation: isolated build and acceptance plan — 2026-10-07

## Decision and evidence boundary

A defensible next experiment is a reviewed EBX zero-extension guard built into a
**complete isolated Git for Windows distribution**, compared with an otherwise
identical unguarded distribution. This note specifies that experiment; no SDK,
package, executable, DLL candidate or virtual-machine setting was changed or
executed by this research.

The [48-row standalone capture](../../../target/docs-checker-register-capture-20261007.1/CPUID-OBSERVATION.json)
preserves every nonzero seed's upper RBX bits at leaves 0, 1 and 13. The separate
[12-command native capture](../../../target/docs-checker-register-capture-20261007.1/RESULT.json)
contains four failed processes, whose allocator uses RBX high32 equal to 1 or 10.
The [PE correspondence](../../../target/docs-checker-register-capture-20261007.1/PE-INSTRUCTION-CORRESPONDENCE.json)
binds CPUID, full-width copy/subtraction and the faulting stack write to installed
DLL SHA-256 `2ea49553e4c03055dcf1c4a2bef54668081a07663fba283f4b34cf70f2157191`.
These are the observed mechanism and discriminator, not attribution to Parallels
or any driver. The [previous source note](2026-10-07-msys-native-docs-checker-fault.md)
retains the installed revision and alignment-fix evidence.

At the fresh API capture `2026-10-07T02:31:44.8262928Z`,
[MSYS2 PR #378](https://github.com/msys2/msys2-runtime/pull/378) is open, draft and
unmerged. Its [one-line change](https://github.com/msys2/msys2-runtime/pull/378/files)
adds `movl %ebx,%ebx` immediately after CPUID and before copying RBX to RCX.
The proposal is experimental; its author's TwinCAT observations concern another
host. Raw API bodies are in the [new receipt packet](../../../target/msys-remediation-acceptance-20261007.1/SOURCE-RECEIPT.json).

## Official prerequisites and reproducible identities

Git for Windows documents an SDK MSYS shell and `sdk build msys2-runtime` as the
runtime build entry point. Its rebuild instructions require closing every
process using the destination runtime before replacing the DLL. Apply that rule
only to the future isolated build root.
[Official runtime build instructions](https://gitforwindows.org/building-msys2-runtime.html)

For local package changes, the documented preparation/build sequence is
`makepkg --nobuild -s`, edit the prepared sources, then
`makepkg --noextract --noprepare`. The first command can install dependencies.
MSYS package metadata lives in Git for Windows' MSYS2-packages repository.
Source-file changes require updated checksums when incorporated into the recipe.
[Official package-management instructions](https://gitforwindows.org/package-management.html)

The following remote references were resolved once and retained; they describe
prospective build inputs, **not acquired or verified release binaries**.

| Input | Exact captured identity | Receipt |
| --- | --- | --- |
| SDK snapshot | `446cf6ac6a95198d1ee2de64948bd3e2dbcc230f` | [SDK reference](../../../target/msys-remediation-acceptance-20261007.1/sdk-main-ref.json) |
| MSYS2 package recipes | `39014c49183d3635c93151c390604318d2655da1` | [commit API](../../../target/msys-remediation-acceptance-20261007.1/packages-main-commit.json) |
| Runtime recipe | `msys2-runtime 3.6.10-4`, x86_64 | [PKGBUILD](../../../target/msys-remediation-acceptance-20261007.1/runtime-pkgbuild.txt) |
| Recipe's embedded runtime commit label | `5a1665c8a0fb24930e55f1621441dfd98a798c15` | [commit-label file](../../../target/msys-remediation-acceptance-20261007.1/runtime-source-commit.txt) |
| Portable packaging helpers | `13d5fbd51585a0f6f21129ec3e9d0f8235aff4b8` | [commit API](../../../target/msys-remediation-acceptance-20261007.1/build-extra-main-commit.json) |
| Proposed guard | PR head `34e9927ef854052e4fef9bc147c93462ba9d735d`, base `3ea87a506e64e841dc0b8ee50ed61999c8fed26c` | [PR files API](../../../target/msys-remediation-acceptance-20261007.1/pr-378-files.json) |

The captured runtime recipe fetches the Cygwin `cygwin-3.6.10` tag, applies its
listed MSYS patches, and embeds the separate commit-label file. That label alone
does not prove the resulting source tree. Resolve the tag to its commit, retain
all patch/checksum inputs, hash the prepared tree and generated assembly, and
compare them with the reviewed source. The
[pinned Git for Windows gendef](https://github.com/git-for-windows/msys2-runtime/blob/5a1665c8a0fb24930e55f1621441dfd98a798c15/winsup/cygwin/scripts/gendef#L232-L240)
still copies full RBX and already rounds the allocation to 64 bytes; its
[captured text](../../../target/msys-remediation-acceptance-20261007.1/candidate-base-gendef.txt)
contains no proposed guard. The recipe's build dependencies are cocom, Git, Perl,
MSYS GCC, the MinGW cross CRT/GCC/zlib, zlib/libzstd/gettext/libiconv development
packages, autotools, xmlto and docbook-xsl.
[Exact recipe](https://github.com/git-for-windows/MSYS2-packages/blob/39014c49183d3635c93151c390604318d2655da1/msys2-runtime/PKGBUILD)

After separate environment authority, use this bounded preparation:

1. Create a new SDK root with no spaces, separate from both the installed Git and
   this SDK repository. Use the official SDK bootstrap/clone route at the pinned
   SDK revision; preserve the installer/archive identity if that route is chosen.
   [Official SDK bootstrap](https://gitforwindows.org/updating-your-sdk.html)
2. Freeze the build package set: retain `pacman -Q`, package ownership/listing for
   Bash and the runtime, architecture/environment, package databases, archive
   SHA-256 values, recipe revisions, compiler/binutils/make versions and complete
   build logs. Resolve every automatically installed dependency before freezing.
   Do not perform an unrecorded package upgrade between the paired builds.
3. Prepare one reviewed source tree. Build an unguarded package and a guarded
   package from equivalent fresh trees, differing only in the reviewed guard and
   explicit package provenance suffix. Do not transplant the entire upstream
   MSYS2 fork over Git for Windows' patch set. Retain the exact patch, recipe
   change, source diff and produced package/DLL hashes.
4. Inspect the emitted x64 instruction sequence in the actual candidate DLL:
   `CPUID; MOV EBX,EBX; MOV RCX,RBX` before allocation and buffer clearing.
   The corresponding guard bytes are `89 db`; candidate RVAs must be discovered
   anew. Do not reuse the installed DLL's RVAs as proof for a different binary.
5. Stage each package set in its own complete distribution. The official
   `./portable/release.sh <version>-test` route collects **installed SDK files**;
   compiling a DLL without placing it in the packaging input is insufficient.
   The captured script supports UCRT64 and MINGW64 distinctly and writes
   `etc/package-versions.txt`; record the chosen environment and package list.
   [Packaging overview](https://gitforwindows.org/technical-overview.html),
   [pinned portable script](https://github.com/git-for-windows/build-extra/blob/13d5fbd51585a0f6f21129ec3e9d0f8235aff4b8/portable/release.sh)
6. Retain the complete PortableGit artifact and extracted-tree manifest, with
   launcher, `usr/bin/bash.exe`, `usr/bin/msys-2.0.dll`, relevant utilities and
   configuration hashes. Verify that every MSYS process in each run loads its
   intended DLL from that distribution. Use an explicit launcher path and a
   child-process environment; preserve the machine PATH and installed runtime.

This yields traceable, repeatable inputs; it does not assert byte-identical
reproducible builds. The inspected recipe has no `check()` function. A package
build exit 0 therefore is not an automatically passing runtime test suite.

## Predeclared acceptance matrix

Use the unchanged [original checker](../../../tools/docs/check_links.sh), SHA-256
`5cbc626ce1fbfc2676c8533cd914358984b2ec49538d2462c800b846d1f496d2`.
Historical native evidence belongs to SDK HEAD
`7174f09f0b34d082bc584180ba4260d53f379d6c`. During this research HEAD advanced to
`c72c907d1f12583fe5072642519c2549f5f0c317`; the checker hash remained identical.
Freeze the full acceptance corpus and its hash manifest separately before
execution; a changing HEAD or link corpus invalidates a paired comparison.

Keep command arguments `-x tools/docs/check_links.sh`, working directory,
fixture bytes, host state, input producer and timing mode fixed. Only the
explicit launcher/distribution changes in the native pair. Collect native exit,
stdout/stderr bytes and hashes, process tree/module paths and hashes, elapsed
time, producer identity/status, extracted-link input count/hash, validation count
and any debugger events. A shell exit 0 without completed input and validation is
not a successful checker observation.

| Stage | Predeclared bounded runs | Required result / interpretation |
| --- | --- | --- |
| Standalone guard discriminator | Original probe and guarded probe, each the same 48 leaf/seed rows | Guard preserves low EBX, clears high32 after the guard; leaf 13's observed `0x340` gives allocation `0x388`. This only validates guard semantics outside MSYS. |
| Original-command native pair | 12 installed-baseline and 12 guarded-candidate runs without debugging, then 12 each with the same late-attach capture method | Candidate completes every validation with native exit 0 and no AV; each debugger attachment failure is explicit. Historical counts are context, not fresh baseline substitutes. |
| Build-version control | 12 unguarded-isolated runs in each of the same two timing modes | Required because this captured 3.6.10-4 recipe differs from installed 3.6.9. Unguarded failure matching the mechanism and guarded success support guard effectiveness; otherwise version/timing effects remain unresolved. |
| Broken local link | Three runs per native arm, adding one retained Markdown fixture pointing to a unique nonexistent local target | Exit 1 and a diagnostic naming the target, with no AV. Remove it only from the disposable fixture between stages. |
| Producer hard failure | Three empty-output and three partial-output runs per native arm, using a hashed process-local `rg` fixture that exits 2 | Require observable producer failure and nonzero checker result. Keep the original checker byte-identical; label a 0 result as a failed completeness control. |
| Empty successful corpus | Three runs per native arm with an empty link set and real `rg` exit 1 | Distinguish legitimate no matches from producer exit 2; checker status 0 is acceptable only with the empty corpus established. |

Do not retry until success or discard failed ordinals. Record the complete fixed
counts even if the baseline is quiet; a quiet baseline limits discriminatory
strength. A matching unguarded native failure plus candidate passes, emitted
guard/source identity and verified DLL loading establish **bounded candidate
runtime effectiveness on this host**, not population reliability or vendor root
attribution. A separately authorized debugger observation of the actual
candidate allocator can strengthen correspondence; standalone guarded assembly
cannot substitute for executing the candidate DLL and original command.

The original checker producer ends in `rg ... || true` inside process
substitution, and its final status tracks missing links. Reading that source
predicts that a producer exit 2 can be suppressed even when the runtime is
healthy. This research did not execute the control. If observed, preserve it as
a failed completeness gate; a runtime-only change cannot justify declaring the
whole documentation gate trustworthy. Any producer-status checker repair needs
its own reviewed repository change and controls, while the original-command
runtime experiment remains separately reported.
[Original source](../../../tools/docs/check_links.sh)

## Exact vendor lead and remaining authority

Parallels' official release notes associate **26.3.0 (57392), March 25, 2026**
with corrections for Git Bash failing to launch in Windows 11 VMs and the MSYS2
installer crashing. They supply a concrete vendor comparison lead, but do not
name CPUID, RBX, XSAVE allocation or this observed fault location.
[Official Parallels 26 release notes](https://kb.parallels.com/en/131014)

The guest's virtual-platform model does not establish the host's Parallels
product/build, host architecture/macOS, or causation. Obtain those exact
read-only identities from the host owner before choosing a supported comparison.
If the host predates the named correction, an approved comparison with that
vendor-supported correction is evidence-driven; if it already contains it, the
published correction supplies no reason to assume another arbitrary version
repairs this mechanism. Rerun the same standalone CPUID discriminator and
original-command fixed counts before/after one approved host change, retaining
the installed runtime hash. Do not combine a vendor update with the DLL guard in
one attribution experiment.

The unperformed environment authority is precise:

- Acquire/bootstrap the pinned SDK and dependencies, build the reviewed runtime
  packages, package/extract complete isolated distributions, and execute those
  candidates in explicitly named disposable directories.
- Run the fixed-count checker and synthetic producer controls in a frozen
  disposable corpus; separately authorize any debugger attachment to its owned
  candidate processes if not already covered by the native-capture authority.
- For the vendor path, approve the exact host Parallels build/Tools update or
  specific vendor-prescribed configuration action, including any shutdown/reboot
  and a recoverable VM snapshot. No such action is needed merely to write or
  review this note.
- Posting captures or an upstream report requires explicit external-message
  authority; replacing the installed Git/DLL or changing machine PATH would be
  a separate adoption decision after acceptance.

No market-data credentials, SQL access, Provider calls, RPC or service control
are prerequisites for this experiment. Gates A–D remain applicable: this note is
a design/evidence proposal, not release approval.

## Receipt and verification scope

Only this note and the new
[receipt directory](../../../target/msys-remediation-acceptance-20261007.1/SOURCE-RECEIPT.json)
were written. Official source responses are retained as UTF-8/LF text with one
final newline; their URLs, capture times and HTTP status are recorded. The SDK
full-commit API retrieval returned a warning prefix and failed JSON parsing;
output-budget truncation is the working explanation, with no complete response
retained. That failed retrieval is recorded, followed by the smaller reference
endpoint that resolved the SDK revision. Previous notes/captures were not
rewritten. The unavailable startup autonomous-long-task skill had already been
disclosed by the coordinating agent; no install or repeated search occurred.

Verification checks this note's local links, receipt JSON parsing, hashes and
whitespace. It does not build or execute a runtime candidate, run the native
acceptance matrix, pass the original docs checker, or establish a released fix.
