# CFFEX 2026 equity-index futures delivery evidence (R-08)

Checked 2026-09-27. This is a source audit, not a monthly delivery notice and not evidence that future deliveries have occurred.

## Finding and evidence classes

The CFFEX IF, IH, IC and IM product contract tables each say that the final trading day is the third Friday of the expiry month, postponed for a national statutory holiday; delivery is on that final trading day and is cash-settled. The product codes are respectively IF, IH, IC and IM. These are **standing contract rules**, not twelve individual 2026 delivery announcements: [IF](https://www.cffex.com.cn/cn/hs300.html), [IH](https://www.cffex.com.cn/cn/sz50gzqh.html), [IC](https://www.cffex.com.cn/cn/zz500.html), [IM](https://www.cffex.com.cn/zz1000/). The [IM detailed trading rule, article 9](https://www.cffex.com.cn/cn/ssxz/20220718/43093.html) is more explicit: if a holiday **or abnormal circumstances** prevents trading on the third Friday, the next trading day becomes both the last trading and delivery day. The [CFFEX risk-control rule](https://www.cffex.com.cn/cn/ssxz/20230414/43079.html) also permits adjustment of contract last-trading and delivery-related dates under exceptional circumstances. Thus a static calendar is conditional on no subsequent exchange adjustment.

The [State Council's 2026 holiday notice, 国办发明电〔2025〕7号](https://www.gov.cn/gongbao/2025/issue_12406/material/gwygb202532.pdf) establishes the relevant 2026 holidays: Spring Festival February 15–23 and Dragon Boat Festival June 19–21. Its government-hosted [HTML republication](https://zwfw.gansu.gov.cn/huixian/zczx/tzgg/art/2025/art_715c16a75e4d4c289c295e77772c7274.html) provides searchable text. February's third Friday is February 20, so the first regular trading day after that holiday is February 24; June's third Friday is June 19, so the first regular trading day after that holiday is June 22. For the other ten months, the third Friday is not in a listed national holiday closure.

The current adapter's `https://www.cffex.com.cn/cn/jystz/20251217/46425.html` is reported as CFFEX's 2026 **holiday closure** notice (中金所发〔2025〕55号), not a monthly equity-index futures delivery notice. Direct retrieval of that CFFEX page failed in this research environment (web fetch timeout and local TLS handshake failure); therefore its exact body has **not** been independently verified here against the primary page. In any case, a holiday notice must not be labelled or used as a *per-month delivery announcement*. If retained, it should only be a separately identified closure-calendar input, paired with the standing product rules. The source/origin of a calculated date must be explicit; `notice_url` must not imply a monthly announcement that was not consulted.

## Rule-derived 2026 schedule, not final delivery confirmation

| Month | Third Friday | Holiday adjustment | Rule-derived planned delivery date |
| --- | --- | --- | --- |
| 01 | 2026-01-16 | none | 2026-01-16 |
| 02 | 2026-02-20 | Spring Festival closure through Feb 23 | 2026-02-24 |
| 03 | 2026-03-20 | none | 2026-03-20 |
| 04 | 2026-04-17 | none | 2026-04-17 |
| 05 | 2026-05-15 | none | 2026-05-15 |
| 06 | 2026-06-19 | Dragon Boat Festival closure through Jun 21 | 2026-06-22 |
| 07 | 2026-07-17 | none | 2026-07-17 |
| 08 | 2026-08-21 | none | 2026-08-21 |
| 09 | 2026-09-18 | none | 2026-09-18 |
| 10 | 2026-10-16 | none | 2026-10-16 |
| 11 | 2026-11-20 | none | 2026-11-20 |
| 12 | 2026-12-18 | none | 2026-12-18 |

The first column's year-month gives the IF/IH/IC/IM suffix `26MM`, according to each product table. Dates above were independently calculated from the third-Friday rule and the cited holiday notice. They do **not** establish that each contract was listed, that each past month actually settled on that date, or that a future extraordinary closure will not alter the date. As of 2026-09-27, October–December deliveries have not happened. A final/confirmed-delivery contract must not silently present those dates as observed fact. If the API cannot express a *planned, conditional* schedule with rule-and-calendar provenance, fail closed for unverified months until an actual CFFEX monthly delivery notice or equivalent primary settlement evidence is retained.

I could not obtain stable first-party CFFEX URLs for all 2026 monthly *post-delivery* notices from this environment. The [CFFEX exchange-notice index](https://www.cffex.com.cn/cn/jystz.html) shows that these are a separate notice class (e.g. February 24 and March 20 notices in the indexed snapshot), but the index result was stale and the underlying pages timed out. Do not manufacture monthly notice URLs from a date or article number.

## Engineering implication

Choose and version one of two semantics before deployment: (1) *rule-derived planned calendar*: include product-specific rule URLs and the government/CFFEX closure-calendar source, explicitly mark the dates provisional/conditional, and document a refresh/override path for abnormal closures; or (2) *confirmed delivery fact*: admit only months with individually retained first-party post-delivery proof, and return a non-success for others. A `complete=true` response carrying twelve unconditional month values and the 2025-12-17 holiday notice as each row's `notice_url` does neither.
