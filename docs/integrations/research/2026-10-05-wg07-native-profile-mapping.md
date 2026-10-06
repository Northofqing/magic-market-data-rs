# WG07 native source-profile mapping — 2026-10-05

## Decision

No production Hithink Fuyao source profile satisfying the supplied WG07 contract is established by the public source material inspected here. This is a concrete source-evidence gap, not an instruction to add an always-unavailable production operation. The already delivered Mac caller deliberately rejects unregistered production profiles before filesystem/network access. Keep that boundary until a real native grammar and its admission evidence exist.

There is nevertheless a useful, connected SDK slice: accept, validate and retain the four documented native date fields on Fuyao ticker search without promoting them into Core listing/lifecycle qualification. The existing `SecurityMetadata` handler already calls this SDK path. This is a shape-compatibility change that can be tested offline; it is not a WG07 production profile and does not prove that the live response currently contains those fields.

## Requirement and evidence boundary

The fully read requirement is the public handoff at:

`\\Mac\Home\Desktop\Quant\stock_analysis\client-bundle\mac-wg07-caller-contract-20261005.1\inputs\grpc_handoffs\2026-10-03-wg07-ordinary-daily-change-window-contract.md`

Its SHA-256 is `56bf90fdce71f948fff1eb1d2c8f389f0f9984a8a2745448005607bd394ddf69` (11,098 bytes). Lines 3–5 and 18–46 define the important distinctions: a synthetic test protocol, separate public `prepare_window`/`consume_window` entry points, an explicit inclusive window, native reconstruction, exhaustive terminal/session evidence, availability/revisions and same-source lifecycle. Lines 60–66 define independent resource and live-admission boundaries. This researcher did not rerun the packet's whole member/ZIP verification; the main agent's verification is separate from the individual files hashed below.

The public caller sources actually inspected are under that same package's `inputs` directory:

| File / lines | Actual seam or requirement | SHA-256 / bytes |
| --- | --- | --- |
| `src/data_gateway/ordinary_daily_change_window_contract.rs`, 258–278 | Production profile registry empty; `HithinkFinance` appears only in the `cfg(test)` `TEST_CODE_SYNTHETIC` profile. | `f27e2efcdfed1d95f87ce97d856f04288ab6e2d158bdbacd50ffbdc6be6a9f29` / 52,759 |
| Same file, 583–655, 775–857 | Availability, correction, revision, session and lifecycle DTOs; only `TEST_CODE_NATIVE_WINDOW_V1` native interpretation exists. | Same exact file/hash |
| `src/data_gateway/ordinary_daily_change_window.rs`, 702–758 | Real library entry points validate the profile before opening existing review storage and acquiring a window. `consume_window` reacquires and uses the same review-state owner. | `96f43a9c0dcbcfde6305714803141cc4f47df5ca74d0c11497e283bfd56b9584` / 28,789 |

The package's manifest was independently hashed here as `2e005f3dd98b8f4f56d2d954bfc42f2fdd176b01276e068504344e16863dd74e`. Its `REPORT.md` says the export freezes existing inputs rather than delivering a standalone compilable SDK. It also preserves omitted legacy `--days=60`. The explicit WG07 `from`/`to` contract does **not** define a former `days=90` request as either 90 civil days or 90 trading sessions; that old selection policy remains a separate decision. The contract binds the checked SSE calendar and explicitly does not establish Shenzhen applicability.

### Evidence levels used

- **Public vendor document/source:** parameter/field meanings and client implementation, pinned to an exact official Git revision. A sample in a document is not an acquired market response.
- **Repository implementation:** what the SDK and current server actually preserve, reject and expose at the inspected HEAD `62502520c75feee34bf9ed67aaa846f60b9d3948`. It is not new network acceptance.
- **Existing observation output:** an already retained provider result and its transport-body receipt. It proves the recorded output, not possession of the raw response body, native historical finality or PIT.
- **Synthetic caller fixture:** useful only for local protocol mechanics; never used as a Hithink native field/profile definition.

## Official Fuyao sources, pinned and reproducible

The official repository's `main` was resolved through its public GitHub commits API to `3bca7805a4127ece8d81961917e740d2effac6ec` during this research. The relevant files were fully read by decoding the official Git blobs, hashing the decoded bytes, and printing their raw line numbers. That is a documentation revision, **not** a market-data snapshot/revision identifier.

| ID | Official original / raw line ranges used | Git blob | Decoded bytes / SHA-256 |
| --- | --- | --- | --- |
| F1 | [A-share prices](https://github.com/HiThink-Tech/Financial-API/blob/3bca7805a4127ece8d81961917e740d2effac6ec/docs/api/a-share/prices.md#prices-historical--历史-k-线), raw lines 106–182; [raw original](https://raw.githubusercontent.com/HiThink-Tech/Financial-API/3bca7805a4127ece8d81961917e740d2effac6ec/docs/api/a-share/prices.md) | `630fd9232398fc196f7b5afff3189b392f3e7333` | 6,131 / `bcb558e44dbe9dfc589945c48894479ea8d15b36d6cc63782ab24b6faa6164a9` |
| F2 | [Ticker search](https://github.com/HiThink-Tech/Financial-API/blob/3bca7805a4127ece8d81961917e740d2effac6ec/docs/api/meta/tickers-search.md), raw lines 15–25, 56–103; [raw original](https://raw.githubusercontent.com/HiThink-Tech/Financial-API/3bca7805a4127ece8d81961917e740d2effac6ec/docs/api/meta/tickers-search.md) | `c76f73179dac0ba27a9c326ce79dd7f638a17023` | 3,628 / `152e67ca6cf00f62d95c531d9f306a38f00c77430a95c0b5cef66084b040d26a` |
| F3 | [Adjustment events](https://github.com/HiThink-Tech/Financial-API/blob/3bca7805a4127ece8d81961917e740d2effac6ec/docs/api/a-share/corporate-actions-adjustment-factors.md), raw lines 5–25, 67–88; [raw original](https://raw.githubusercontent.com/HiThink-Tech/Financial-API/3bca7805a4127ece8d81961917e740d2effac6ec/docs/api/a-share/corporate-actions-adjustment-factors.md) | `7c01ebbb2f61ae08a7111798651106ddbfa27df4` | 3,195 / `d3167326a0b963d96082bcb09d16a8aa39a25d0f95ccd408d3ef0fe513ba5045` |
| F4 | [Trading calendar](https://github.com/HiThink-Tech/Financial-API/blob/3bca7805a4127ece8d81961917e740d2effac6ec/docs/api/a-share/calendar-trading-days.md), raw lines 5–22, 58–72 | `c6dd566e475e02f8d8fa67539d05451606a402a1` | 1,771 / `b6a85fc753d722cb27fa1a1852bfdc5a5b38d69a123805c6e7c89bf6a313ff52` |
| F5 | [Official marketdb schema](https://github.com/HiThink-Tech/Financial-API/blob/3bca7805a4127ece8d81961917e740d2effac6ec/python/marketdb/sql/schema.sql), raw lines 11–58 | `b55af9d65384276a0872747654bf0bbddb5eeb2f` | 2,746 / `3b4131339bac492d236af729aa8fd9bb8e4f975020343cec1b9bb75101f80d2b` |
| F6 | [Official daily updater](https://github.com/HiThink-Tech/Financial-API/blob/3bca7805a4127ece8d81961917e740d2effac6ec/python/marketdb/updaters/daily.py), raw lines 115–157 | `7a26b11aac172c656da5f5b5657b099ed26190c6` | 5,844 / `4c9c1a4f5d6ae96a79b5dd7218e472549445f709d9eead038bd9be0274c5372a` |

The live documentation landing page `https://fuyao.aicubes.cn/docs/` was inaccessible to the web-reading tool. The pinned official repository originals above were accessible, including the ticker-search raw URL. No inaccessible page or search snippet is treated as field evidence. F1 is a module file; the guessed `docs/api/a-share/prices/historical.md` path did not exist in the pinned tree.

## Field-by-field WG07 mapping

R1–R6 below identify exact repository evidence; their hashes follow the table. F1–F6 identify vendor originals above.

| WG07 obligation | Actual native/source fact and current preservation | Qualification decision |
| --- | --- | --- |
| Single Equity / venue | Existing observation has `native_response.thscode=688561.SH`; R1 validates exact response code, and R2 captures it. F1's response field table/sample, however, lists only `timestamp`/`item`, not context echoes. | Positive recorded native identity evidence for that observation; not a full documented/versioned WG07 grammar. Derive `.SH` venue only under a reviewed suffix interpretation, never from the outer request alone. |
| Day / unadjusted | Existing observation has `interval=1d`, `adjust={state:Value,value:none}`. R1 requests and checks these; R2 distinguishes omitted/null/value adjustment. | Preserve native distinctions. Never substitute an outer `Unadjusted` tag for an absent echo. |
| Exact native request / range / selection echo | R1 constructs explicit Shanghai midnight through end-of-day millisecond bounds, `adjust=none`, `offset=0`. R2 stores validated final URL and server `request_id`, not source-resolved start/end or selection fields. | A locally recorded URL proves what was sent, not provider interpretation or selected range/as_of. Neither an opaque request ID nor QueryRequest SHA supplies the missing native response link. |
| Native pagination and exhaustive range terminal | F1 documents a ten-year bounded single-security query but no historical page/total/has-more/exhaustion fields. Historical `offset=0` is sent by the SDK but is not listed among F1 historical parameters. R2 says exhaustion `Unknown`. | Cannot produce WG07 complete page manifest or native range terminal; one successful response is not native exhaustion. Do not borrow snapshot endpoint pagination. |
| Expected sessions / calendar | F4 is a parameterless rolling near-year A-share calendar. It does not give this caller's pinned SSE authority hash/metadata or venue-specific qualification. | Continue using the caller's admitted authority mechanism. Calendar dates alone do not explain security-specific missing rows or prove SZSE applicability. |
| One terminal per expected session | R1 validates received bar dates for uniqueness/range and sorts them. No native session terminal field exists in that DTO. R2 says missing-date reasons `Unknown`. | Cannot infer `Suspended`, `NotYetListed`, `Delisted`, a valid bridge, or complete `NoChanges` from omission/zero rows. Bar-only successes do not prove all terminals. |
| Exact decimal lexemes / units / scale | F1 uses numeric OHLC/volume/turnover with price/amount in original currency, A-share CNY and volume shares. R1 decodes them immediately as `f64`, then volume becomes Core lots. | Units are documented, but source lexemes are lost in current normalized output. R2's body SHA cannot recover lexemes. No public scale<=8 guarantee was found; bounded exact parsing must validate actual bytes, never format f64 back into a purported source decimal. |
| Native response bytes / interpreted fact refs | `get_historical` hashes the actual response before decoding, then consumes it. R2 keeps SHA/length/final URL only. R6 old output has a body receipt, not `native_hex`. | Hash binds an absent body, not replayable native facts. Exact lowercase hex/fact-ID interpretation would require bounded body retention in the real SDK path. |
| First availability / publication | R1 treats `data.timestamp` as newest returned bar date; row `source_at` is the bar date. R2 explicitly says historical publication `NotProvided`. | Not first availability; not a publication instant. Shanghai 15:00, local receipt time or bar date cannot fill this field. |
| Native selected `as_of` | Current history query has no as_of parameter/selection proof in the SDK or inspected native context. | `selected_as_of` cannot be minted from WG07 request input. Fresh retrieval does not prove historical selection. |
| Data revision / snapshot / initial | R2 explicitly says revision `NotProvided`; public Git/documentation SHA is not data-version evidence. | Cannot manufacture `Initial`, immutable snapshot IDs or revision manifests from body hash/request ID. |
| Correction / predecessor selection | No historical corrected-version chain or predecessor manifest is preserved by current source shape. | Cannot satisfy `Replaces` closure or know that a current value was the value available at caller as_of. |
| Same-source listing | F2 supplies a nullable `list_date` and a current code-table snapshot load time. R3 currently does not decode its date fields. | Useful same-provider current metadata fact, not an as_of listing interval with native completeness/revision evidence. |
| Same-source delisting | F2's `end_date` means contract expiry, not A-share delisting. Last trading/delivery dates likewise have their own meanings. | Do not relabel any of these as `delisting_date` or a native `Delisted` terminal. |
| Same-source action terms / identity | F3 supplies response thscode/ticker, row ticker/ex-date/cash-per-share/bonus-per-share. R4 validates identity/date bounds, decodes terms as f64 and creates Distribution events. | Narrow native terms/identity exist. Current SDK projection is not native lexeme material or a WG07 lifecycle manifest. |
| Action status / date roles / Complete or None | F3 explicitly says event_type/record_date/adjust_factor are not returned. R4 constructs `Implemented`, effective-on=ex-date and source-time-absent evidence; its coverage is the local request. No native exhaustive/no-event statement is retained. | Do not promote locally constructed status/bounds, or an empty list, to native complete implemented-action coverage. Rights/category/date completeness remains unproved. |
| Availability/revision for listing AND actions | F2 current snapshot loading time is not each historical fact's first availability; F3 has no native timestamp/version selector in R4. | Bars plus current same-provider metadata still fail the all-selected-record availability manifest. Same provider is necessary, not sufficient. |
| Exact protobuf request binding | R5 version-2 envelope hashes original JSON payload bytes; WG07 requires SHA of the assembled complete protobuf QueryRequest and independent native binding. | Keep legacy binding unchanged; it is not the WG07 binding and cannot be relabeled. |
| Failure evidence / resource class | Current SDK has typed errors and four-MiB HTTP bodies. WG07 additionally bounds each native decoded entry at one MiB, cumulative/proof/protobuf at eight MiB, including hex expansion. | No transport allowlist/body widening is justified. A real profile would need bounded exact-byte/error retention under both native and complete-proof limits; current receipts are insufficient for WG07 failed/successful replay. |

This is a mapping of the inspected shapes, not a statement that all undocumented fields are impossible for the vendor to add. There is no successful complete WG07 production mapping from these originals. The decisive missing blocks are native selection/range finality, per-session terminals, availability/version/correction history and complete same-source lifecycle. Exact-decimal retention alone cannot overcome them.

### Repository evidence and source hashes

All relative R paths below are beneath the actual inspected worktree:

`C:/Users/13687/.codex/worktrees/grpc-coverage-20261002/magic-market-data-rs`

| ID | Exact path and inspected lines | SHA-256 / bytes |
| --- | --- | --- |
| R1 | [Hithink SDK](../../../crates/magic-hithink-rs/src/lib.rs), `crates/magic-hithink-rs/src/lib.rs`, 223–298, 441–451, 621–642, 842–872, 927–1015 | `0acf56ff7618c7b639bff186000669a460b86605b9565842ab104f392733cf59` / 59,006 |
| R2 | [Historical observation](../../../crates/magic-hithink-rs/src/historical_coverage.rs), `crates/magic-hithink-rs/src/historical_coverage.rs`, 30–70, 90–158 | `8f7365a19919078daa8d0026fec28789e5f9d73ed4ac6991ec08a58b1f08c684` / 4,590 |
| R3 | [Metadata SDK](../../../crates/magic-hithink-rs/src/metadata.rs), `crates/magic-hithink-rs/src/metadata.rs`, 14–65, 86–102, 115–155 | `cc9aec1c14c84a72c0b38fc8a4cd061458dc6474db3519d8af4afa16d70bb6b5` / 12,288 |
| R4 | [Corporate-action SDK](../../../crates/magic-hithink-rs/src/corporate_actions.rs), `crates/magic-hithink-rs/src/corporate_actions.rs`, 19–41, 62–160 | `d807d878b59e8db153bfbd15ecab55f8ae690d0eb80e6c0950607f1d4e66cc52` / 10,751 |
| R5 | [Existing production handlers](../../../crates/magic-market-composition/src/grpc_production.rs), `crates/magic-market-composition/src/grpc_production.rs`, 1512–1527 and 4641–4694 | `c405e042225142082f4d11e1d831707234a1a7fb067219a81e07badc8d3886f8` / 295,662 |
| R6 | Existing `target/hithink-negative-packet-9e215d7/native-limit-15.json`, fully read | `24135564cd28079c5bebc9807eb38c548104e21a4dbd4ccbca3b1780f5a7b1f1` / 8,880 |
| R7 | [Version-2 boundary](../grpc-historical-bars-coverage-v2.md), `docs/integrations/grpc-historical-bars-coverage-v2.md`, fully read | `d65a6986603efcfaa64ffa8858eb32b1baba17fe8489a14f1bee861b99098d0c` / 3,739 |

R6 records one old 2026-07-16..2026-07-30 Shanghai equity observation with 11 validated/returned rows and no local truncation. Its original transport-body receipt is length 1,752, SHA-256 `404217b93a1d0f8df8fa19dabcaac2e6f1d098e04824ba4086ccf5c6fdbcc537`. These are **not** the bytes/SHA of the normalized R6 file. The output explicitly reports exhaustion/calendar/missing-reasons `Unknown`, publication/revision `NotProvided`, PIT false. No fresh response or archived raw native body is claimed here.

The official marketdb is not a hidden solution to those gaps: F5 stores OHLC/terms as DOUBLE and keys current bars by thscode/date; F6 uses request thscode/currency/interval/adjustment fallbacks when native rows omit them and replaces rows in the current store. Local import batches/clock values are not provider-native historical availability or revisions. Its useful research database contract must not be reinterpreted as this caller's proof grammar. [Official schema](https://github.com/HiThink-Tech/Financial-API/blob/3bca7805a4127ece8d81961917e740d2effac6ec/python/marketdb/sql/schema.sql), [updater](https://github.com/HiThink-Tech/Financial-API/blob/3bca7805a4127ece8d81961917e740d2effac6ec/python/marketdb/updaters/daily.py).

## Immediate SDK slice: native ticker-date shape compatibility

F2's native date meanings are exact and must stay separate:

| Native field | Documented response type | Meaning / format | Document example |
| --- | --- | --- | --- |
| `list_date` | string or null | Listing date, `yyyy-MM-dd` | `2007-03-01` |
| `end_date` | string or null | Contract expiry, `yyyy-MM-dd` | null |
| `last_trade_date` | string or null | Last trading date, `yyyy-MM-dd` | null |
| `last_delivery_date` | string or null | Last delivery date, `yyyy-MM-dd` | null |

These facts are from F2 raw lines 73–76 and 100–103, not live observations. Response omission is **not** declared in that table; nullable is not the same as officially optional. An SDK can explicitly preserve Absent/Null/Value for backward-compatible handling without claiming the vendor promised omissions. F2's `timestamp` is the current code-table snapshot's upstream loading time, not historical listing-fact publication. [Pinned original](https://raw.githubusercontent.com/HiThink-Tech/Financial-API/3bca7805a4127ece8d81961917e740d2effac6ec/docs/api/meta/tickers-search.md).

Request comparison: F2 requires string `q`, accepts optional string `exchange`, optional string `asset_type`, optional integer `limit` (default 10, maximum 50). R3 already sends these same four keys: exact full thscode, suffix SH/SZ/BJ, appropriate leaf asset type(s), limit 50. No new endpoint, query key, credential, listener or transport dependency is needed to support the documented response shape. Exact identity and one-match validation must remain; ticker search is not intrinsically an exact-only search.

R3's strict `TickerItem` presently has only thscode/ticker/name/exchange/asset_type/currency. Since `deny_unknown_fields` is enabled, an otherwise valid response containing any of the four dates will be rejected. This is a deterministic source-shape mismatch, **conditional on those fields being returned**; no live failure has been diagnosed by this task.

Recommended implementation slice, for the main agent to assess under normal Gates:

1. Add explicit typed native state for only these four documented fields at the existing ticker-search DTO. Validate non-null values as bounded canonical dates; preserve omission/null/value, and continue rejecting undocumented fields and wrong JSON types. Do not invent cross-field ordering/delisting rules that the source does not define.
2. Keep the existing `SecurityMetadata` projection/status and admitted capability semantics unchanged in this slice: no `listed_on` promotion, no native `Delisted`, no lifecycle Complete/None, no history as_of. Preserve the native date state in the SDK's actual owned response/outcome if made available; an externally serialized native outcome would require its own reviewed public version, not silent fields on frozen Core records.
3. Test the actual `security_metadata` path, not a disconnected parser/probe: old six-field shape, four-field document shape, explicit null, absence, valid values, wrong type/date/length, unknown fields, identity conflicts, out-of-bound results and existing Core/RPC output invariance. Documentation-based fixtures prove decoder compatibility only. R5 already connects `SecurityMetadata` to this SDK path; no WG07 dispatch is necessary.

This slice repairs a useful production SDK seam while keeping source qualification honest. It does not make the supplied `prepare_window`/`consume_window` production-capable. Do not update the older statement “listing date is unpublished” into a claim “listing/lifecycle is admitted”; the accurate update is “documented native nullable date fields exist, and complete historical lifecycle evidence is still unproved.”

## Next connected history slice and exact vendor gaps

If further source SDK work is authorized, reuse R1's real `historical_bars_with_coverage` acquisition and R5's existing reader boundary. A lossless native outcome can retain the credential-free request target, bounded exact response body and original decimal tokens **before** serde/f64 normalization, plus truthful known/missing context. Keep v1/v2 wire compatibility unless a separately reviewed, versioned public evidence contract is agreed with its actual consumer. Do not add an unconsumed observation script, duplicate service method, default-routing fallback or synthetic production grammar. The transport remains the closed existing policy; WG07's larger complete-proof bound is not permission to enlarge source HTTP limits.

This is only an evidence-preservation slice, not a complete WG07 profile. The vendor source must still supply an authoritative native contract and actual original evidence covering:

- Response-bound native identity/venue/adjustment plus exact resolved request/query/range/selection identity; historical pagination/exhaustion or an explicit exhaustive non-paginated range terminal.
- Every requested expected session's Bar/Suspended/NotYetListed/Delisted terminal and any permitted suspension bridge, without deducing missing reasons from a short list.
- First availability with precision/timezone, source-selected as_of, version identity, explicit initial evidence and corrections with actual predecessor closure for bars **and** lifecycle records.
- Same-source listing intervals and complete implemented-action coverage or explicit no-event coverage, with all material native dates/categories/terms, including source-defined handling of unsupported action kinds.
- Lossless numeric source grammar and units, validated within caller's scale/overflow bounds; immutable native bytes/fact references and bounded complete proof/failure delivery.

Obtaining that material is a separate authorization/admission step, not performed here. Only after it exists should a real source/profile/version be implemented in the Mac caller interpreter/registry and matching source SDK `HistoricalBars` dispatcher, with the exact capability scope and independently verified delivery. The existing WG07 native interpreter explicitly rejects non-synthetic grammar; implementing just the server half would not connect a production consumer.

EMQuant was not expanded into a replacement-source survey in this task. Switching providers or combining Hithink bars with TDX lifecycle cannot fill the above source-bound gaps; an alternate provider would need its own full contract/evidence mapping. Earlier six SZSE snapshots remain feasibility evidence only, not a qualification assembled from other dates/sources. D17, D20 and R08 remain independent unresolved work and are not used to fill WG07 evidence.

## Actual research method and mutations

Read the worktree AGENTS/CONTEXT and research skill; no `.codegraph` directory exists, so used bounded `rg`/PowerShell reads. The required `autonomous-long-task` startup skill was unavailable at the checked local/worktree locations; this task continued as restricted read-only source research, not expanded execution authority. Read the complete WG07 Markdown requirement and selected actual public caller DTO/entry-point source. Read current Hithink integration/coverage documentation, SDK DTO/normalizers and existing production handler, plus the already retained observation output. Resolved the official GitHub revision, read the pinned document/source blobs and computed their decoded-byte hashes; also verified the pinned GitHub/raw document URLs via browser reads. Reviewed the verification-before-completion skill before document checks.

No provider/market-data request, 17709/local-terminal call, capture, credential/config-secret read, service/listener/RPC, deployment or push was performed. Only this Markdown note was added by this researcher; Rust, tests, registry, business rules and endpoint allowlists were not changed. The three existing untracked 2026-10-02 notes were preserved. This is not a test/build/live-acceptance report.
