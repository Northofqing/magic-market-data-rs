# Windows gRPC evidence for Mac continuation

Mac Codex can read this public handoff from
`/Users/<Mac account>/Desktop/Quant/stock_analysis/client-bundle/windows-evidence-20261002.1/`.
The corresponding Windows share is
`\\Mac\Home\Desktop\Quant\stock_analysis\client-bundle\windows-evidence-20261002.1`.
This package contains evidence and contract documentation only. Do not replace
the existing client credentials, production listener, sealed Wave1 inputs or
approved Mac candidate with it. No credentials are included.

## Current production identity

The read-only receipts below came from the same running production process
at `https://10.211.55.3:50051`. Health is live/ready and has no identity error;
process start is `1790862122793`. The exact tuple is:

```text
service_version  0.2.0
source_revision  67c832e43f36f188e4d769f409691c0b1d9a2ea2
contract_sha256  abf28a3e0028488a7579da4d961e1a7c1408482bdc0500122c1956d225e480cf
binary_sha256    9302a3036c7ed272fdf24dd84b4a66881ab682649e9af76b68ef62b2ad083854
```

The public proto has 65 market RPCs; operations 64 and 65 are
OfficialPublications and OfficialPublication, not old DailyBarDiscovery.
Use the already delivered `windows-grpc-20261001.3` public proto/metadata for
this production identity. An old descriptor's different operation 64 cannot
be substituted.

## HistoricalBars input now available

`historical/` contains the complete three same-build JSON receipts, exact
CanonicalRecord payload strings and hashes, request IDs and payload hashes,
before/after Health and Capabilities. Run
`36763c29f80a46b9bef088c717368d66` passed bounded positive acceptance, exit 0.
Decode known complete protobuf fields from the explicitly labelled reencoded
hex if needed; these are not original transport wire captures or native HTTP
bodies. Read `grpc-historical-bars-records.md` before implementing field mapping.

For 2026-07-16 through 07-30 and limit 15, 688277 returned one 07-30 bar;
688561 returned eleven dated bars. The limit 1 control on 688561 returned only
07-30 but **also `complete=true`**. Keep the complete responses immutable;
do not promote that flag to session coverage/PIT, reconstruct missing records
or make the 688277 07-30 bar a source-issued reopen event. Provider-local
native `thscode` is validated by the adapter but not retained in these records;
the normalized instrument is not a raw source-local identity echo. Date gaps,
terminal count, revision/finality and authority-empty remain explicit gaps.

## MoneyFlows and BoardFlows remain failed

`flows/` run `33c1dcff982f4df6a4482cf4802048f2` retains both original typed
failures and raw error trailers. Both RPCs returned `Unavailable` with
`reason_code=provider_unavailable`, despite same-process ready Health and
available capabilities. The source endpoints closed TLS without close_notify.
There are no positive money/board records and the strict flow acceptance is
exit 2. The error trailer's provider is blank and its attempts list is empty
in this production revision; the preferred provider is retained in the request.
Do not infer a recorded provider attempt from the capability alone.

The production stdout/stderr files were zero bytes when checked. There is no
server trace receipt to supplement those request IDs, and no trace ID is
invented. Existing network diagnosis remains applicable; TLS verification,
allowlists and transport bounds were not weakened.

## News evidence and version control

`news/` run `8de4718c2b3c4ccd8af845dd972a33fe` contains one real positive
version 2 record from each of WallstreetCn, Jin10 and legacy Cls. It passed
bounded positive acceptance, exit 0. These first-page records do not prove
keyword recall for Rubin/Muse or archive completeness. Use news request and
record schema version 2; version 1 is explicitly refused and is not an upstream
reachability result. Further enabled-provider controls will be delivered as
separate evidence, not overwrite this run.

## D14 D17 D20 source controls

`authority-controls/` contains nine original public source bodies and their
manifest. Read `native-coverage-contract-controls.md` for issuer/date/page/error
controls. SZSE supplies distinct two-day positive issuer rows, but a query over
five days returns HTTP 200 plus a native rejection and zero totals; that is not
empty success. An empty page past the reported last page is not query-empty.
SSE's different issuer/date empty queries return byte-identical bodies without
native filter echoes. Neither control repairs TDX identity or proves exhaustive
daily lifecycle/all-action absence. These research endpoints are not newly
admitted runtime fallbacks.

## Successor implementation and pending prerequisites

CNInfo coverage v2 is being verified in a separate worktree based on the exact
production source. The provider's bounded prefix, caller truncation and duplicate
overlap must report incomplete; a v2 envelope adds original request hash,
page/count/terminal/hash evidence. The protobuf remains unchanged. Its new
implementation will require its own source/binary identity and successor
candidate; this evidence package does not advertise v2 as currently deployed.

Windows still needs genuine same-build flow positives, the isolated real
CNInfo pagination receipt, D14 native TDX identity/exact-terminal evidence,
D17/D20 complete lifecycle/daily/all-class authority contracts and R08
independent monthly confirmed originals with correction/finality rules. Mac
owns its immutable history consumer, strict evidence acceptance, self-contained
credential bundle and independently approved successor deployment. No existing
Wave1 approval is inherited by these unfinished inputs.
