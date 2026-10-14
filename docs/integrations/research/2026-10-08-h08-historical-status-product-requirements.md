# H08 historical trading facts: products, rights and original deliveries, 2026-10-08

## Conclusion and authority boundary

This is a product-requirements investigation, **not delivered historical data,
an account-entitlement audit, Gate A approval, implementation or deployment**.
The strongest concrete Shanghai candidate is SSEINFO's **行情历史数据 / 智能数据
“历史数据” subscription**, with status-bearing daily snapshots and a separately
specified securities-basic-information delivery. Shenzhen's official **FTS
静态交易参考信息 and Binary 现货集中竞价交易快照行情** identify the necessary
fields, but the historical archive SKU, retention/backfill service and rights
for those exact fields remain unconfirmed. These differences follow the
[SSEINFO historical product](https://www.sseinfo.com/services/assortment/historical/),
[intelligent-data product v2.0.0, §2](https://www.sseinfo.com/services/assortment/znsj/znfwnr/c/10790553/files/01e3eccd5e4f41a3a0f960b859fb47e8.pdf),
[SZSE XML v1.43, §§2–3](https://www.szse.cn/marketServices/technicalservice/interface/P020260918541117849849.pdf)
and [SZSE Binary v1.17, §§3–4](https://www.szse.cn/marketServices/technicalservice/interface/P020250328368568358456.pdf).

No OHLC, zero volume, missing bar, absent public suspension event, current
security-name prefix or present-day company profile is promoted to historical
Trading/Suspended, ST, board, lifecycle or issuer identity. Admission and
source-proven-empty requirements remain governed by
[BR-007](../../business_rules.md#L145), [BR-009](../../business_rules.md#L154),
[BR-021](../../business_rules.md#L175), [BR-032](../../business_rules.md#L29)
and the [engineering gates](../../ENGINEERING_RULES.md#L2).

The target is the **1,076 securities** in the original read-only package
`\\Mac\Home\Desktop\Quant\stock_analysis\client-bundle\mac-outcome-data-diagnostic-20261007.1`:
`REPORT.md:1` reports 2,702 candidate sample windows and an October 9 T+5 end;
independent parsing of `instruments.json` returned 1,076 entries. Neither count
is a delivered fact count. The package manifest SHA-256 is
`a4617473d47b5e69660207fe44fe0892e07d71b29dbad4a393b4b0afcf30a1a4`;
`instruments.json` is
`25734b59c2134a82804a048d27218b7b3814e631a3ecd0b8049eb8230c0d6079`.
Only this package's report, manifest and instrument identifiers were read;
no new 1,076-security API scan, SQL execution or production probe was performed.

Coverage requested is July 2–September 24, 2026, plus actual T+1/T+3/T+5 dates.
The original package's verified calendar and the exchanges'
[SSE holiday notice](https://www.sse.com.cn/disclosure/announcement/general/c/c_20260915_10832273.shtml)
and [SZSE holiday notice](https://www.szse.cn/www/disclosure/notice/general/t20260917_622911.html)
give September 28, September 30 and October 9 respectively. The local research
cutoff is **2026-10-08 13:30 +08:00**: October 8 is not yet a completed session,
and October 9 is future. Neither date is prefilled as completed history;
calendar membership alone proves no instrument-level state.

## 1. Concrete official product and access map

| Candidate actually identified | Documented machine delivery / fields | What is still not established |
| --- | --- | --- |
| SSEINFO 行情历史数据, historical product v2.0.0 completed 2025-06-12; interface v1.1.5 completed 2026-03-27 | Daily `sh1/yyyymmdd/Snapshot.csv` or `sh2/yyyymmdd/Snapshot.csv`, all securities in one file; L1 `SecurityID`, `DateTime`, `PhaseCode`, `MsgSeqNum`, `SendingTime`; L2 `InstrumentStatus`, `TradingPhaseCode`, `NumImageStatus`. UTF-8 CSV. | Actual July–October files, historical universe/completeness receipt, publication/correction history, PIT/as-of delivery and account rights. No archive was acquired. [Product §2](https://www.sseinfo.com/services/assortment/market/hqywwd/wdcpsms/c/10782125/files/f2ba70dea74a4323bf13b76fffce0e40.pdf); [interface §§3.1.1.1, 3.2.1.2, physical pp.4–7,10–13](https://www.sseinfo.com/services/assortment/market/hqywwd/wdzsjk/c/10824356/files/4ddcd5a592f9429f9288b0d2cb0c9549.pdf). |
| SSEINFO 智能数据 v2.0.0 completed 2025-01-03: “历史数据”, “每日行情数据”, cloud “云数聚” | History includes securities basic-information files; cloud history is accessed as agreed files. Daily increments synchronize by **Rsync at 18:00 after each trading day**; spec names 《Rsync 数据同步服务用户手册》 and 《行情历史数据文件结构说明》 as necessary companion manuals. | Those manuals, exact basic-information filename/schema, historical inventory, actual delivery times and rights to export cloud files locally were not obtained. A schedule is not a record's historical availability time. [§§2.1–2.3, physical pp.3–4](https://www.sseinfo.com/services/assortment/znsj/znfwnr/c/10790553/files/01e3eccd5e4f41a3a0f960b859fb47e8.pdf). |
| SSEINFO LDDS 静态数据 v1.2.0 completed 2026-06-11; SSE IS124 market-data-file spec v3.70 dated 2026-09 | `cpxx0201MMDD.txt`, `cpxx0202MMDD.txt` and other static files; LDDS table schedules the first group for trading-day 08:00. `cpxx0201` contains numeric upper/lower prices, tick, listing date, security subclass and product-status bits. STEP UA2001 carries raw file content in tag96. | Live/static access is documented, **not licensed historical backfill**. SSEINFO must confirm whether the historical basic-information subscription actually delivers these file families/schema or a different equivalent. Current v3.70 is not proof of every July day's effective schema. [LDDS §§2–4](https://www.sseinfo.com/services/assortment/market/hqywwd/wdzsjk/c/10821683/files/b2118cc05a974203a549e47ce3ed370d.pdf); [IS124 §2.8, physical pp.14–20](https://www.sse.com.cn/services/tradingtech/data/c/10832592/files/98a374cfa623415181710783a1718c28.pdf). |
| SZSE FTS 静态交易参考信息, data-file interface v1.43 published 2026-09-18 | Broadcast business `szse_marketdata`; dated `pre_securities_YYYYMMDD.xml`, then authoritative `securities_YYYYMMDD.xml`; `cashauctionparams_YYYYMMDD.xml` supplies cash-auction parameters. Also specified reception via satellite MDGW for listed file families. | Participant/access-network semantics are not a public history download API. No authorized archive product name/order form, historical retention inventory, correction replay, daily originals or this user's FTS rights were obtained. [§§2–3, physical pp.18–23,34–35](https://www.szse.cn/marketServices/technicalservice/interface/P020260918541117849849.pdf). |
| SZSE Binary 现货集中竞价交易快照行情 v1.17 published 2025-03-28 | `MDStreamID=010`, message `300111`; `OrigTime`, `SecurityID`, `SecurityIDSource`, `TradingPhaseCode`; `xe`/`xf` entries carry actual upper/lower price information, including no-limit sentinels. | Live protocol does not identify a historical SKU, archive layout or current-account right. Snapshot messages cannot be retransmitted; tick-channel retransmission/end messages do not certify snapshot-history completeness. [§§3.2–3.3,4.5.4, physical pp.11–12,22–28](https://www.szse.cn/marketServices/technicalservice/interface/P020250328368568358456.pdf). |
| 深圳证券信息有限公司 / 深证信数据服务平台: 数据API, 个股F10/公司概况, 数据浏览器“基本资料”, 企业图谱 | The operator's [CNINFO site](https://www.cninfo.com.cn/) links these named services and [company list](https://www.cninfo.com.cn/new/snapshot/companyListCn). The public [F10/company page](https://webapi.cninfo.com.cn/shgs/company.html?companyid=002183) exposes labels for company name, A/B/H securities, market and listing date. | The inspected page returned **labels without populated records**. The API portal returned a JavaScript shell; no primary API operation/schema documenting a stable issuer ID, historical security-to-issuer intervals or PIT/version fields was obtained. Do not invent a `p_sysapi...` operation or treat `companyid=002183` as a proven legal-entity ID. |

The SSEINFO [business entry](https://www.sseinfo.com/services/assortment/historical/)
names telephone 4008888400, key 3-1, and its business-application platform.
LDDS §2 explicitly requires existing LDDS connection **and a contract with
SSEINFO**, followed by IDC permission activation. No application, login,
credential inspection, purchase or permission change was made. Public PDFs
do not establish either our entitlement or a paid archive's availability.

For Shenzhen, CNINFO's owner-operated navigation links
[行情服务](https://www.szsi.cn/cpfw/fwsq/hq/hlsdkfw.htm) and
[行情授权公示](https://www.szsi.cn/cpfw/fwsq/hq/index.htm).
Both failed this host's normal TLS handshake in the bounded public GETs below;
their **contents and any historical SKU/terms remain unverified**, not absent.
The [joint exchange authorization statement](https://www.sseinfo.com/aboutus/authstatement/)
names SSEINFO and 深圳证券信息有限公司 as the respective operators and directs
users to apply for permission. CNINFO's
[company-list legal statement](https://www.cninfo.com.cn/new/snapshot/companyListCn)
distinguishes noncommercial browsing/download from written permission for
sale/profit-oriented copying, storage, electronic collection or redistribution.
These are first-party statements, not a legal clearance or license for H08.

## 2. Field semantics: what the originals would have to prove

### Shanghai

L1 historical `PhaseCode` is an eight-character source field: character 1
includes `P` suspension; character 2 distinguishes normal tradability (`0/1`);
character 3 distinguishes listed (`0/1`). These are direct state fields, not
OHLC inferences. Snapshots are spaced 3/5 seconds and consecutive identical
snapshots are compressed to the first. Consequently a gap is not automatically
a suspension or packet loss; a daily reduction needs a documented full-session
coverage rule. [Historical interface §3.1.1.1, physical pp.4–7](https://www.sseinfo.com/services/assortment/market/hqywwd/wdzsjk/c/10824356/files/4ddcd5a592f9429f9288b0d2cb0c9549.pdf).

IS124 `cpxx0201` field 14 is the first trading/listing date; field 21 is the
price tick; fields 23/24 are actual upper/lower values. **Only limit type `N`
has actual controlling upper/lower prices**; other types' numeric values are
reference-only. `P` denotes IPO no-limit treatment, `R` other specified
no-limit treatment, and `U` no price control. Security subclass distinguishes
`ASH` main board and `KSH` STAR; product-status character 4 marks normal,
risk-warning or delisting-period products. A risk-warning bit is not license
to infer all historical ST subtypes from a display name. [IS124 §2.8, physical
pp.15–20](https://www.sse.com.cn/services/tradingtech/data/c/10832592/files/98a374cfa623415181710783a1718c28.pdf).
The cpxx table explicitly marks its ISIN field **reserved**; neither a reserved
field nor a security-code/request echo qualifies an issuer identity.

The latest files' listing date does not supply a certified historical delisting
transition or `covered_through`. Likewise, the spec's “last trading date” in
this table is defined for treasury pre-issuance products, **not a universal
equity delisting date**. Obtain equity lifecycle originals separately. The
v3.70 change log includes a July 2026 risk-warning-limit amendment, but its
month label is not an exact effective-day attestation for a security. Preserve
each day's source schema/rule version. [IS124 change log and §2.8](https://www.sse.com.cn/services/tradingtech/data/c/10832592/files/98a374cfa623415181710783a1718c28.pdf).

### Shenzhen

Formal `securities_YYYYMMDD.xml` covers listed securities except specified
temporary business codes. It carries `SecurityID`, source `102`, `ISIN`,
`ListDate`, `SecurityType`, and repeated `SecurityStatus/Status` values such as
`1` suspended, `4` ST, `5` *ST, `6` first listing day and `10` delisting period.
`SecurityType` directly distinguishes main-board A shares (`1`), ChiNext (`3`)
and main-board B shares (`4`). The preliminary file can differ and must not
replace the formal file. Its dated filename identifies **effective trading
date**, not first publication or knowledge time. [XML §§3,3.1, table 3-2,
physical pp.20–23,32](https://www.szse.cn/marketServices/technicalservice/interface/P020260918541117849849.pdf).

`cashauctionparams` gives `PriceTick`, per-session `PriceLimitSetting/Type`
(`O/T/C`), `HasPriceLimit`, `LimitType` (`1` percentage, `2` absolute),
`LimitUpAbsolute` and `LimitDownAbsolute`. Therefore “this XML has limit
parameters” does **not** prove actual absolute bounds were populated on a
particular day. Obtain actual controlling absolute values or companion original
Binary `xe`/`xf` entries; do not calculate limits downstream from percentages.
Do not confuse the separate effective-order-price-cage parameters with a daily
limit band. [XML §3.4, physical pp.34–36](https://www.szse.cn/marketServices/technicalservice/interface/P020260918541117849849.pdf).

Binary `TradingPhaseCode` character 0 has `H` temporary suspension, while
character 1 has `1` full-day suspension and `0` normal status. A normal state
at one instant is not a full-day Trading attestation. Binary `FinancialStatus`
is instead the early/midday listed-company disclosure prompt (`A/B`), **not
suspension status**. For `xe`, `999999999.9999` means no upper limit; for
nonnegative-price securities `xf` equal to the tick (stock example `0.01`)
means no lower limit. Those positive numbers are still sentinels, not genuine
bounded prices. [Binary §4.5.4 and field dictionary, physical pp.23–28,44](https://www.szse.cn/marketServices/technicalservice/interface/P020250328368568358456.pdf).

Post-close `cashsecurityclosemd_YYYYMMDD.xml` does not replace those sources:
its no-trade blank prices / zero turnover are not a suspension field. Download
completeness must not be inferred from upload `.flag`/MD5/`.suc` conventions
for participant uploads. [XML §§2.3,5.1, physical pp.18–19,62](https://www.szse.cn/marketServices/technicalservice/interface/P020260918541117849849.pdf).

## 3. Public historical positive examples, not a complete status product

The original official [July suspension table](https://docs.static.szse.cn/www/market/periodical/month/W020260806351615093691.html)
contains 22 published rows and the [August table](https://docs.static.szse.cn/www/market/periodical/month/W020260903544741911834.html)
20. Examples: July `002036` has July 23 09:30 suspension and July 30 09:30
resumption; August `002084` has August 17 09:30 suspension and August 24 09:30
resumption. Local exact-ID membership check found **002084** in the original
H08 `instruments.json`; the other checked illustrative IDs (`002036`, `000595`,
`301139`, `002183`) were not in that file. This was local identifier comparison,
not status or issuer acquisition for the population.

The August table also publishes `9999/12/31 00:00` for unresolved resumption
labels, including `301139`. Preserve that sentinel; do not create a real future
date or automatically extend coverage into September/October. These tables
have instrument, event times and reasons but no exhaustive daily state matrix,
row revision/PIT history or certified absence. In particular **absence from
these event tables cannot construct an empty suspension-window set**. The
August 24 resumption instant is also not an inclusive suspended-through date:
a daily representation needs an approved session/boundary convention.
The parent task's original web-reader failures for both table URLs remain
failures; later native GETs and existing original captures are separate evidence.

## 4. Precise mismatch with the current Mac consumer

Read-only source:
`\\Mac\Home\Desktop\Quant\stock_analysis\src\data_gateway\qualified_trading_facts.rs`,
SHA-256 `4819ade4788ef91a360a37f98788aa78b41c4b7ff4ee284d78c9b8801464aa9d`.
This code was inspected statically, not run or changed. Production `acquire`
at lines 744–752 still returns `ContractNotDelivered`.

| Current contract requirement | Product/schema evidence is not sufficient until… |
| --- | --- |
| `AuthorityLifecycle`, lines 131–162: `listed_on`, `delisted_on` = **first date no longer listed**, `covered_through` | Dated source originals certify the exact equity lifecycle and complete transition coverage through each requested date. `ListDate`, a current list or a last-trading-date label alone is insufficient. |
| `QualifiedPriceBand`, lines 185–227: authority actual upper/lower prices, tick, explicit board/ST, effective interval, nonempty version | Daily numeric values are delivered with limit-type semantics, correct units/tick alignment and effective/source version. Percentage parameters cannot be substituted. |
| Price constructor, lines 209–217: positive lower/tick and upper ≥ lower | IPO/unbounded states and positive no-limit sentinels require an **independent Gate A contract decision**, not fabricated or gigantic “valid” bands. |
| `AuthoritySuspensionCoverage`, lines 284–340: bounded coverage; inclusive date windows; no matching window ⇒ Trading | Source proves the whole coverage interval. A sparse public event table, missing file or incomplete snapshot run cannot create `trading(...)`. Requests beyond coverage remain Unavailable (lines 646–668). Intraday H/P events require an explicit daily-reduction meaning. |
| Evidence time validation, lines 405–421 and 580–594: offset RFC3339 `source_at`/`observed_at`, source ≤ observed | Time meaning/timezone is sourced, not guessed. Date-only announcements cannot gain an invented midnight; business/event time is separate from historical publication/availability time. |
| `AuthorityTradingFactsRecord` and `QualifiedTradingFacts`, lines 540–563 | **No issuer field exists here.** Issuer identity needs a separate source and contract; this investigation has not audited 1,076 issuer records for missingness. |

The daily state meaning itself needs Gate A approval: full-day suspension,
status at a defined instant, status at close, or existence of any trading within
the session are not equivalent. The chosen meaning must preserve intraday
suspension/resumption and lifecycle transitions rather than silently collapse
them. This is a proposed design question, not approval or implementation.

## 5. Requested deliverable originals and source-owned metadata

The following is a **proposed supplier delivery bill**, not a statement that a
supplier has delivered it. Each row follows the source fields above and the
current consumer at lines 131–340, 540–594.

| Deliverable to request | Exact scope/content necessary | Permission/contract evidence still needed |
| --- | --- | --- |
| SSEINFO historical status package | Every required completed SSE trading date: raw `sh1/.../Snapshot.csv` or `sh2/.../Snapshot.csv`, correct effective schema, status field presence, all required securities and session coverage, day/file manifests/counts/checksums and source-declared missing/empty reasons. Choose L1 vs L2 explicitly; OHLC-only files are not substitutes. | Product/order ID, historical date inventory/backfill entitlement, named account grant and daily completeness semantics. |
| SSEINFO historic securities-basic-information companion | Source-confirmed archive filenames/schema and raw daily originals, ideally controlling cpxx-equivalent absolute prices/tick/board/ST/listing facts. If the live cpxx family is offered, retain full business year as well as MMDD and all corrected generations. | Written confirmation whether the historical subscription includes these exact fields, not merely “basic information”; separate static-feed rights if required. |
| SZSE/SSIC authorized historical reference archive | Actual dated **formal** `securities_YYYYMMDD.xml` and `cashauctionparams_YYYYMMDD.xml`, source effective version, pre/formal relationship, full records and correction originals. | Concrete offered archive SKU/API/file channel, historical retention and backfill bounds, participant or vendor authorization for these exact files. The live protocol alone does not establish an archive. |
| SZSE/SSIC historical state + actual band archive | Original Binary `010/300111` state and `xe/xf` values, or a licensed equivalent source-produced daily/state history; preserve generation times, intraday transitions, raw no-limit semantics and completeness receipts. | Archive layout/schema, compression/loss/finality rules and source-owned history/revision/as-of policy. Tick retransmission cannot certify snapshots. |
| Equity lifecycle ledger, both markets | Instrument/exchange/ISIN, listing effective day, exact first nonlisted day, relisting and security-code changes, authoritative transition originals, complete `covered_through` per security, event/effective dates and versions. | Supplier's explicit completeness population and interpretation of last trade vs termination vs first nonlisted day. Null delisting alone is not perpetual listing. |
| Issuer/entity history, separate from security facts | Source-owned stable entity identifier, exact security→issuer mapping with effective intervals, legal name/renames and sourced legal identifiers where provided; original API/file schema, sample response and revision/publication history. Security code/name is not a stable legal-entity key. | Specific CNINFO/SSE or authorized product/API operation and schema, historical mapping rights and PIT retention. Public F10 labels and empty company pages do not deliver this. |
| Source publication and correction ledger | Original generation, first publication/availability and revision publication timestamps; source timezone rules; version/replacement IDs, effective date intervals, correction supersession and as-of query/delivery samples. Preserve local observation and HTTP metadata separately. | Source-owned interpretation and precision, historical replay availability and an actual original+corrected sample. Latest corrected truth alone cannot prove what was known at the decision time. |

As a documented revision mechanism, LDDS UA2001 provides `CategoryID`,
`MsgSeqID`, `DataTimeStamp`, `FileName`, `FileStatus`, fragment count/index and
raw bytes. Intraday correction emits a new message rather than mutating the old
one; larger sequence selects the latest same-topic message **within that day**,
and sequence numbering restarts daily. This supports retaining original and
corrected deliveries, but is not a global correction ID or proof that today's
history subscription preserves all generations. [LDDS §4, physical pp.8–9](https://www.sseinfo.com/services/assortment/market/hqywwd/wdzsjk/c/10821683/files/b2118cc05a974203a549e47ce3ed370d.pdf).

Business `DateTime`/`OrigTime`, send time, file effective date, HTTP Date,
Last-Modified, ETag, local hash and receipt time must keep their own meanings.
None independently certifies historical first-available time or finality.
The source's `LocalTimestamp` / numeric timestamp must retain its raw form
until timezone and precision are documented. This respects the consumer's
validation and [BR-033](../../business_rules.md#L46).

Procurement/authorization questions still requiring the operator's written
answer (no message was sent):

1. Which exact subscription/order covers historical status, static reference,
   lifecycle and issuer mapping for this population and July 2–October 9?
   Quote separate missing families rather than promising a generic “行情” SKU.
2. Does the grant permit automated non-display research/backtesting, Windows/Mac
   storage of raw and normalized facts, retention of superseded versions, derived
   diagnostics and gRPC/client redistribution? What are expiry/deletion,
   user/device, location, concurrency and rate constraints?
3. If supplied by a reseller, provide the actual upstream authorization for
   **historical** data and each family/field/date range, not just a live-display
   authorization logo. Which entity is the licensee and does this account have
   the requested grant? Neither was inspected here.
4. Provide a small original trial bundle and source receipt, including a
   full-day suspension, ordinary Trading with complete coverage, intraday
   resume, ST/board transition, no-limit IPO, delisting boundary, and one
   original→correction/PIT pair before any full-population acquisition.

Those are authority-boundary questions; this note does not spend, authorize,
activate a feed or contact the supplier. Provider admission and transport
changes still require approved Gate A and matching
[admissions](../admissions.tsv) / [HTTP transport registry](../http-transports.tsv).

## 6. Bounded proposed conformance cases (not executed source tests)

For Gate A review and later implementation, propose deterministic fixtures
bound to **source-delivered originals**, not guessed normal states:

- Full-day suspended vs explicit normal state; one normal snapshot must not
  certify the full session. Intraday H/P and resumption-at-open exercise the
  approved daily reduction and exclusive event vs inclusive date boundary.
- Missing day, missing security, preliminary-only XML, truncated CSV/XML,
  incomplete fragmented file and uncertain terminal/count evidence all remain
  explicit Unavailable/failure; no fallback to bars or empty windows.
- Request at coverage end succeeds only with matching source evidence; next
  date fails. October 8 before completion and October 9 before occurrence are
  not completed historical fixtures.
- Genuine finite absolute bands pass with tick/units and version; percentage-
  only parameters, mismatched O/T/C regimes, source-price-cage substitution,
  noncontrolling SSE limit types and SZSE no-limit sentinels do not pass as
  bounded `QualifiedPriceBand`.
- Lifecycle first-listed/last-listed/first-nonlisted boundaries, historical
  ST/board changes and issuer rename/relisting mappings retain separate dates
  and identities; absent issuer data cannot fill a nonexistent gateway field.
- Same-day correction order, daily sequence reset and schema-version changes
  preserve superseded originals. Corrected latest state must not leak into an
  earlier PIT cutoff without recorded publication/availability evidence.
- Missing source timezone, date-only publication, malformed timestamps or
  source after observation fail the current evidence contract; HTTP/local
  timestamps cannot manufacture source time.

These proposed cases are derived from the exact protocols and consumer
requirements in §§2,4–5. They are not live coverage, newly passing tests or a
capability advertisement.

## 7. Original evidence, changed attempts and remaining gaps

Existing public originals were reused from
`C:\Users\13687\.codex\worktrees\grpc-coverage-20261002\magic-market-data-rs\target\daily-trading-authority-research-20261007.1`.
The parent also prepared their shared transport location
`\\Mac\Home\Desktop\Quant\stock_analysis\client-bundle\windows-h08-source-materials-20261008.1\originals`;
this researcher confirmed the shared bundle directory exists, not a new
historical-data delivery or license.
Their matching `*.capture.json` preserves the **2026-10-07** capture times;
derived `*.extracted.txt` is not the original PDF. This task independently
recomputed the following six hashes; reuse is not a fresh 2026-10-08 source
publication. Exact primary URLs are linked above.

| Original | Bytes | SHA-256 |
| --- | ---: | --- |
| `sse-historical-1-1-5.pdf` | 739656 | `2772ee17e7392cd5faf6df15dc76cb542027075ed402b8ad4b3977b4837cac23` |
| `sse-files-3-70.pdf` | 932600 | `fcf039131f1e70f0f6d29fe51cc4b55554c7c628f7ff96d039ae7d1c7d1e5fd5` |
| `szse-files-1-43.pdf` | 960730 | `ae97af16581642d22872e4005995513e32f18d3b751f35a05cb286e661a69159` |
| `szse-binary-1-17.pdf` | 484573 | `e15a70e356420c7aca00abfc29750717a35c5213a521c24e402ff52ea8885c6d` |
| `szse-july-suspensions.html` | 7493 | `5e1657ecd3c5c67d62e986b9bb3c8be9bd3a8b347fead27b6fc9c5b877a69275` |
| `szse-august-suspensions.html` | 7505 | `f82f75a52f61f08c2223fd6329fb25d0b04f75fb1857c81959cfb06e64b6ac23` |

New bounded credential-free native GET observations on 2026-10-08 used normal
certificate verification, no redirects/cookie reuse, 20-second HttpClient
request/header timeout and
16-MiB cap for the two SZSE PDFs/monthly bodies. The XML/Binary and monthly
responses were HTTP 200 at **05:24:32–05:24:41 UTC**, with the same four body
hashes above. Bodies were processed in memory, not saved as a new evidence
bundle. Observed responses completed in less than two seconds each; this
in-memory research capture is not a verified production whole-body deadline
policy or transport admission. The initial PDF console extraction had wrong output encoding and an
`UnicodeEncodeError`; it did not alter the originals. Later reading existing
UTF-8 extracted text supplied the precise fields. Initial monthly decoding
failed to reveal the table; later GBK decoding of the original correctly
revealed it. A `bs4` attempt failed `ModuleNotFoundError`; stdlib decoding and
HTML stripping then recovered the displayed August rows without installation.

The newer SSEINFO intelligent-data URL ending `01e3eccd...pdf` and LDDS static
URL ending `b2118cc0...pdf` were successfully read as official specifications
by the web reader on this task. **No new local raw copies/hashes were created
for those two.** The distinct older intelligent-product URL ending
`1e1427df...pdf` remains the 2026-10-07 HTTP 404/150-byte observation in its
existing receipt, SHA-256
`d03c9c9c5fef19b7e84125dd79b77167d00e1df5f56da4818992e23e67360bbb`.
Finding a different valid current URL does not erase that failure.

Additional changed-method outcomes, not source-data acquisition:

- Web opens of the two SZSI service/authorization pages and SZSE English
  data-services page timed out. Default-TLS native GETs of the two SZSI links
  then failed SSL establishment at **05:28:44 UTC** (1-MiB cap, 20-second
  timeout); no HTTP response/status/body was obtained, and TLS was not weakened.
- `https://webapi.cninfo.com.cn/` returned HTTP 200, 4,138 bytes at
  **05:28:45 UTC**, SHA-256
  `350741d7137dfbbd1570b9e77e251ad07688ef4314d5d718825a302ea582ec44`:
  JavaScript application shell, not API documentation or issuer records.
  An alternate UI attempt failed because the in-app browser was unavailable;
  browser inventory was empty. No authentication/challenge workaround was used.
- Web-reader screenshot of IS124 physical page 16 failed. This does not deny
  the source table: its unchanged local original and page-labeled extraction
  supplied the cited field definitions. Earlier web-reader PDF/monthly failures
  remain distinct from later native HTTP success.

Precisely **not obtained**: any target-date daily status snapshot/reference
archive; actual price-band originals for the population; source-certified
daily completeness or lifecycle `covered_through`; historical delisting
transitions; populated stable issuer mappings/API schema; historical
publication/availability and original→correction samples; current account
entitlement or redistribution/storage grant; the Rsync/file-structure manuals;
the Shenzhen historical archive SKU and delivery contract. Product facts and
public event examples are useful procurement evidence, not substitute datasets.

## 8. Verification and unchanged-state scope

This researcher used `autonomous-long-task`, `research`, the PDF-reading skill
and `verification-before-completion`; this is the assigned background researcher
and spawned no further agents. Applicable AGENTS and engineering/business
rules were read; CodeGraph was consulted before code inspection and did not
contain the Mac-only contract symbols, so the known Mac source was read directly.

Only this new Markdown belongs to the current task's write scope. The earlier
Eastmoney note and existing evidence folders were preserved. There were no
source, registry, contract, credential, SQL/database, schedule, service, binary,
public-bundle, commit, push or deployment changes. Formal Windows runtime source
`841e4ae7a9df62be0c4536fa9009c66089d282bf` was not operated or replaced.

One direct PowerShell→Bash check invocation reported nonzero without diagnostic
output; it was retained as a check-entry failure, not ignored. The changed
explicit-shell invocation
`bash --noprofile --norc -c 'bash tools/docs/check_links.sh; rc=$?; echo docs_check_exit=$rc; exit "$rc"'`
printed `docs_check_exit=0` and exited 0. Fresh document verification:
eight local citation targets/line bounds passed; `git diff --check` exited 0;
the new file had zero tabs or trailing whitespace. Tracked diff remained empty;
the single new note and pre-existing untracked items were separately inspected.
The final SHA-256 is reported to the parent after the last check. These checks do not certify source coverage,
entitlement, Rust tests, Clippy or release gates. The remaining Gate A choices
are the daily state meaning, no-limit representation, issuer contract and the
explicit licensed product/access/delivery/PIT design—not a proposal to weaken
the current unavailable state.
