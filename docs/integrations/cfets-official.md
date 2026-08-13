# CFETS integration

## Capability state

Shibor, LPR and official FX fixing admission flags are true after bounded live
and serial-load admission. DR007 remains false and fails before I/O.

The 2026-08-13 review found a technically reachable DR007 display feed, but no
terms-permitted public machine-data contract. Reachability is not admission:
CFETS written authorization is required before software may fetch, retain or
process this market data.

## Official host and paths

Only the audited HTTPS CFETS/China Money public rate and central-parity routes
declared in the crate endpoint policy are permitted.

## Request and response ceilings

JSON responses are capped at 2 MiB, requests start at one-second intervals,
FX history is limited to 20 pages and 1,000 rows.

## Identity, unit, missing, and source-time semantics

Shibor has exactly eight tenors; LPR has exactly 1Y and 5Y. DR007 is not R007
or Shibor. FX parsing requires the complete closed 25-heading catalog and
preserves base/quote orientation and quotation base, including 100JPY/CNY.

DR007 is also distinct from FDR007. The official money-market page labels the
former as the weighted rate for the `DR007` pledged-repo product. FDR007 is a
separately calculated fixing and must not be substituted for it.

## Public-display contract audit

The 2026-08-13 page implementation referenced two unauthenticated display
assets. Its latest JSON row carried `productCode=DR007`, `weightedRate`,
`latestRate`, `avgPrd` and a page-level `showDateCN`; the official note says the
deposit-institution market display updates every 15 minutes. Its chart CSV
contained recent dates and positional DR001/DR007/DR014 values.

These observations prove identity and explain what a human sees, but they do
not form a stable historical API contract. The CSV has no header or schema
version, its column meaning exists only in page JavaScript, and it exposes a
rolling display window rather than requested date bounds, total count or
completeness evidence. Neither asset documents revisions, correction policy,
retention, pacing or machine-use rights. Independently, the legal terms below
prohibit unlicensed electronic acquisition. The resource paths therefore are
intentionally absent from endpoint policy and source code.

## Authentication or usage-rights boundary

No member login or private trading endpoint is used by the admitted Shibor,
LPR or FX families. Operators remain responsible for source terms and
redistribution rights.

DR007 is blocked by a stricter boundary. The China Money legal declaration
states that CFETS owns website market data and prohibits electronic scraping,
copying and dissemination without prior written authorization. The official
information-product notice additionally prohibits unlicensed copying,
transmission, storage, use, processing and derivative works. Therefore the
public website's dynamically loaded files are discovery evidence only; they
are not approved crate endpoints and must not be called by a diagnostic or
production client.

Official evidence:

- [China Money legal declaration](https://www.chinamoney.com.cn/chinese/legaldeclaration/)
- [Information products and application notice](https://www.chinamoney.com.cn/chinese/xxcpjjjsq/)
- [Money-market pledged-repo display](https://www.chinamoney.com.cn/chinese/mkdatapm/)
- [CFETS data-interface application guide](https://www.chinamoney.com.cn/chinese/dataInterfaceService/)
- [CFETS data-information services overview, March 2026](https://www.chinamoney.com.cn/dqs/cm-s-notice-query/fileDownLoad.do?contentId=1716546&mode=open&priority=0)

## DR007 authorization path

The only candidate direct path identified by the official material is a
licensed CFETS information-product service over the local-currency CMDS
interface, not scraping the public display. The repository has not received a
licensed product catalog proving that the applicant's entitlement includes
DR007; that inclusion must be confirmed in writing. The official guide limits
the applicant to an eligible interbank local-currency market member legal
entity or authorized branch; funds and similar non-legal persons apply through
their investment manager. The applicant uses the
[interbank market account system](https://ibrs.chinamoney.com.cn/AAMS) and
completes service activation, interface testing and production launch.

Before Gate A can reopen, the operator must:

1. obtain written permission identifying DR007 latest/history, intended use,
   storage, retention and any redistribution;
2. sign the applicable interface agreement and CFETS information-product
   licence, and obtain test and production entitlement;
3. obtain the official schema, endpoint, authentication, pacing, history
   coverage, revision and entitlement documentation directly from CFETS;
4. provide redacted authorization and contract evidence to this repository,
   without committing credentials; and
5. request an endpoint-policy and HTTP-registry review before implementation.

The official guide lists `4009787878-2-5` for interface applications,
`4009787878-5-1` for interface development/testing and
`4009787878-5-2` / `cmdssupport@chinamoney.com.cn` for CMDS information-product
acceptance. Redistribution through a product or public service requires a
separate information-provider or media authorization; ordinary endpoint access
does not imply redistribution rights. An ineligible direct applicant may
contract with a CFETS-authorized information provider, but that vendor's
identity, schema, licence and endpoint would require its own Gate A review and
must not be relabelled as the existing direct CFETS provider.

## Deterministic tests

Fixtures cover exact tenor order, complete FX heading order, pagination,
duplicate dates, pair orientation, empty-response rejection and fail-before-I/O.

## Live and load admission evidence

On 2026-07-29, Shibor, LPR and official FX each passed two bounded live probes
for `2026-07-20` through `2026-07-29`, followed by its three-call serial load
probe with at least one second between actual request starts. On 2026-08-13,
the official public page was observed to display DR007 latest and rolling
history data, but its website terms do not authorize machine acquisition.
No authorized DR007 endpoint, schema, entitlement or usage contract was
available to the repository, so no diagnostic/live/load call was made and the
capability remains false.

## Explicit unsupported operations

DR007 history and latest observations, realtime quotes, inferred quotation
bases and partial/empty strict batches are unsupported. DR007 continues to
return a typed `Unsupported` error before I/O in both formal and probe paths.
