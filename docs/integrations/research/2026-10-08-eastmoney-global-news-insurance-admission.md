# Eastmoney GlobalNews: exact insurance-host metadata admission candidate

Research date: 2026-10-08, Asia/Shanghai. Status: **Gate A candidate only; not approved, implemented, admitted by this note, or deployed.** The only file written by this research is this note. The existing Windows gRPC runtime, binaries, public client bundles, contracts, registries and credentials were not changed.

## Finding and decision required

The exact first-party finance rolling page currently emits a `财经` article link on `insurance.eastmoney.com` with the existing numeric `/a/<id>.html` path. The current complete-page parser rejects that hostname because it is absent from the ten exact hosts in BR-020. A narrow metadata admission is supported by the fresh first-party observation below; an Eastmoney-wide wildcard or a new article-fetch endpoint is not supported. [Official finance rolling page](https://roll.eastmoney.com/finance.html), [BR-020](../../business_rules.md#L494), [current host check](../../../crates/magic-eastmoney-rs/src/news.rs#L276).

The proposed Gate A decision is: add **only** `insurance.eastmoney.com` to the BR-020 canonical metadata host list, retain `/a/<nonempty ASCII digits>.html`, retain scheme-only normalization to HTTPS with the source hostname and path unchanged, and retain complete-page validation before the caller limit. Approval must also name the matching business-rule/integration-evidence/registry recording plan. This is a decision for the owning reviewer, not a statement that the scope has already been approved. [Gate A](../../ENGINEERING_RULES.md#L2), [AGENTS HTTP authority boundary](../../../AGENTS.md#L23).

The sampled HTTPS insurance article returns a redirect to the same article ID on `finance.eastmoney.com`. Under the existing metadata-only contract, production does not fetch or resolve this link, so that observation does not require enabling redirects or replacing the stored hostname. If the reviewer instead requires `canonical_url` to identify the final HTTP landing URL, that is a different contract decision and requires its own design and evidence; it is not part of this candidate. [Metadata boundary](../../business_rules.md#L494), [normalization implementation](../../../crates/magic-eastmoney-rs/src/news.rs#L276), [fresh redirect observation](#fresh-first-party-observations).

## Original failure: exact receipt, with missing upstream URI preserved

The safe original receipt is `\\Mac\Home\Desktop\Quant\stock_analysis\client-bundle\windows-grpc-production-841e4ae-20261008.1\evidence\news-Eastmoney.json`, read directly for this research. Its SHA-256 was independently recomputed as `d9b005f4eae99eba8fa683068c5ab3c73b0d069f35f00562d4c86d114e0a7c89`. The receipt is a native raw RPC diagnostic, not Mac consumer acceptance or message delivery, as its own `scope` states on line 24.

| Receipt field | Exact observed value | Owning location |
| --- | --- | --- |
| Started / completed UTC | `2026-10-08T03:54:27.9832897Z` / `2026-10-08T03:54:28.3227922Z` | receipt lines 4–5 |
| Shanghai wall clock | 2026-10-08 11:54, UTC+08:00; converted from the receipt UTC timestamps | receipt lines 4–5; local conversion, not source evidence |
| Request ID | `windows-compatibility-20261008-Eastmoney-57e9e503bcb74e7cac673f332656da64` | receipt line 8 |
| Operation / versions | `magic.market.global_news.request`; protocol 1, payload schema 2 | receipt lines 9, 12, 14 |
| Request data | base64 `eyJsaW1pdCI6Mn0=` decodes to `{"limit":2}` | receipt line 13 |
| Admission / Provider selection | `allow_unadmitted=false`, `preferred_provider="Eastmoney"` | receipt lines 17–18 |
| Native result | `grpcurl_exit_code=73`, `response=null`, `qualified_for_mac_production=false` | receipt lines 20–23 |

The exact caller-visible failure on receipt line 22 is:

```text
ERROR:
  Code: FailedPrecondition
  Message: Eastmoney protocol error: Eastmoney news article host "insurance.eastmoney.com" is not an admitted global-news host
```

The receipt does **not** contain the original offending article URI, its row position/title/time, the raw upstream rolling-page bytes or their digest. The fresh sample below was published at 12:23, after this 11:54 failure, and therefore is not the original failing article. The original missing URI/body evidence remains missing; no later sample is substituted for it. Source: the exact receipt cited above, lines 1–25.

The parent task identifies the formal runtime as source `841e4ae7a9df62be0c4536fa9009c66089d282bf`, descriptor SHA-256 `abf28a3e0028488a7579da4d961e1a7c1408482bdc0500122c1956d225e480cf`, and server SHA-256 `7407a03b744dc4a0040257c5c8de4cef49ec283d5ddc3529fe4506ef04832a4f`. Local `git rev-parse HEAD` independently returned that source commit. Descriptor/server identity and continued process health were supplied by the parent, not independently probed by this research. The parent also supplied bundle-root manifest SHA-256 `859ee3108af179ca79cd99e5e8a16a64c8cae4748f95059a5d310a36c7b56347`; this research recomputed the individual failure receipt hash, not that manifest. These distinctions prevent this note from becoming a deployment or runtime-health attestation.

## Fresh first-party observations

These are new credential-free research observations, separate from the original RPC. The requests used a .NET `HttpClientHandler` with redirects, cookies and proxies disabled; no TLS validation callback or bypass was installed. Each request had a 12-second timeout, a cancellation token covering streamed reads, and a 2 MiB response cap. No environment variables, private keys, browser session state or authenticated API were read. Only this note, selected metadata and response digests were retained; full raw bodies were not archived as fixtures. Consequently the hashes identify the observed responses but are not a claim that replayable raw-body originals have been delivered.

| ID | Explicit request / UTC observation interval | Result recorded in this research |
| --- | --- | --- |
| E1 | GET [exact admitted rolling endpoint](https://roll.eastmoney.com/finance.html), `2026-10-08T04:49:12.6006312Z` → `04:49:13.0482878Z` | HTTP 200; `text/html; charset=utf-8`; no Location; 48,682 bytes; SHA-256 `764b3873c8afcdc7322e2cce6bc843c11e8c64b05eadd27fb7e808f1b8a27bde` |
| E2 | GET [same-host HTTPS sample](https://insurance.eastmoney.com/a/202610083889432650.html), `2026-10-08T04:49:57.9216496Z` → `04:49:58.2282994Z` | HTTP 302; `text/html`; Location `https://finance.eastmoney.com/a/202610083889432650.html`; 215 bytes; SHA-256 `142bb7f7b760c6a1a63366e009fa09278b3ffdd3305394ecc9456d57aa4c8a21` |
| E3 | HEAD of the E1 source-supplied HTTP article URI, `2026-10-08T04:49:58.2791321Z` → `04:49:58.3376462Z` | HTTP 302; `text/html`; Location `https://insurance.eastmoney.com/a/202610083889432650.html`; no body was requested or hashed |
| E4 | Separate explicit GET of the [E2 finance Location](https://finance.eastmoney.com/a/202610083889432650.html), `2026-10-08T04:50:20.5714008Z` → `04:50:20.8838646Z` | HTTP 200; `text/html; charset=utf-8`; no Location; 66,516 bytes; SHA-256 `f28e0a74f03687e47aece1353cc374138b826967864fa6ddad86ff5443e2daaa` |

E1 yielded 40 `<li>` rows inside the exact `#artList` region and one insurance-host row. That count came from bounded research extraction, **not** from successful native Provider validation or a GlobalNews RPC. The row had the following source facts; this is a metadata transcription, not a manufactured normalized record:

| E1 row fact | Value |
| --- | --- |
| Publication label | `2026-10-08 12:23` |
| Exact category markup | `[<a href="finance.html">财经</a>]` |
| Source href | `http://insurance.eastmoney.com/a/202610083889432650.html` |
| Title attribute and visible title | `首期1500亿元！财政部发行中央金融机构注资特别国债` |
| Article ID path component | `202610083889432650` |

E1 also contained the direct navigation anchor `<a href="http://insurance.eastmoney.com/">保险</a>`. Independently, the [official finance rolling page](https://roll.eastmoney.com/finance.html) browser-readable navigation links to the [Eastmoney insurance channel](https://insurance.eastmoney.com/), whose page is branded as Eastmoney and presents insurance-industry, regulatory and company-news categories. This is first-party linkage evidence for the specific channel, rather than an ownership assumption from the `.eastmoney.com` suffix alone. E1's actual `财经` row supplies the relevant finance-feed classification; merely appearing in navigation would not establish that classification.

E4 displayed the same article title and a visible publication label of `2026年10月08日 12:23`. Its visible source attribution identified `证券时报`. Thus first-party evidence here concerns Eastmoney's platform, feed classification and link routing; it does not prove Eastmoney authored the underlying article. The current Provider's fixed publisher label is `东方财富网`, and this candidate does not change publisher semantics or create original-author fields. [E4 first-party page](https://finance.eastmoney.com/a/202610083889432650.html), [current metadata projection](../../../crates/magic-eastmoney-rs/src/news.rs#L82).

No `rel="canonical"` element was found by the narrow research extractor in the E2 302 body or E4 HTML, and no selected `og:url` metadata was extracted. This is a scoped observation, not proof that all possible canonical metadata mechanisms are absent. The observed Location header proves a redirect for this sample at that time; it does not establish permanent routing for every numeric article path. Production's `canonical_url` is currently a deterministic HTTPS normalization of the feed href, not a redirect-discovered final URL. [Normalization](../../../crates/magic-eastmoney-rs/src/news.rs#L276).

## Current repository contract and causal path

The main agent fully read `autonomous-long-task` and the repository research skill, then used CodeGraph before locating or reading code because `.codegraph/` exists. The engineering rules, all 1,331 lines of `business_rules.md`, and the complete admissions/HTTP transport registries were read. The following references point to current on-disk code at the source commit above, not to a guessed production implementation.

1. `global_news` rejects a limit over 20 before I/O and fetches only `https://roll.eastmoney.com/finance.html`, with `Accept: text/html` and the fixed Eastmoney Referer. [news.rs:35](../../../crates/magic-eastmoney-rs/src/news.rs#L35).
2. It calls `parse_complete_global_news_page` before `.take(limit)`. Every list group and row must parse; an unadmitted host anywhere in the complete inspected page fails the whole request even when the caller requests two records. This explains the observed host rejection without identifying which row triggered the original failure. [news.rs:61](../../../crates/magic-eastmoney-rs/src/news.rs#L61), [news.rs:100](../../../crates/magic-eastmoney-rs/src/news.rs#L100).
3. A row must start with a valid minute timestamp, contain the exact `财经` category markup and one article link, and have agreeing title-attribute/visible-title text. Publication time is normalized to RFC3339 with `+08:00` while record source evidence retains the original minute label. [news.rs:189](../../../crates/magic-eastmoney-rs/src/news.rs#L189), [metadata evidence](../../../crates/magic-eastmoney-rs/src/news.rs#L82), [BR-054](../../business_rules.md#L889).
4. URL normalization accepts only literal `http://` or `https://`, rejects `?`, `#` and `@`, requires one of ten exact host strings, then requires `/a/<nonempty ASCII digits>.html`. Explicit ports, trailing dots, nested/lookalike hosts, encoded or nonnumeric IDs do not match these checks. The normalized URL is `https://` plus the same host and path. `insurance.eastmoney.com` is absent from the current exact host list. [news.rs:276](../../../crates/magic-eastmoney-rs/src/news.rs#L276).
5. Duplicate numeric item IDs and duplicate normalized URLs reject the complete page; changing a host must not enable the same ID to appear twice. Records retain `summary=None`, `content=None`, empty instruments, topic `财经`, language `zh-CN` and native source evidence. [news.rs:159](../../../crates/magic-eastmoney-rs/src/news.rs#L159), [news.rs:82](../../../crates/magic-eastmoney-rs/src/news.rs#L82).
6. The HTML transport validates the exact rolling-page URL, requires UTF-8 `text/html`, bounds the read to 2 MiB, uses positive connect/read/write timeouts and disables redirects. The default client timeout is 12 seconds; clones share the production request gate with a minimum one-second interval. None of the proposed metadata hosts becomes an article request endpoint. [transport.rs:73](../../../crates/magic-eastmoney-rs/src/transport.rs#L73), [transport.rs:233](../../../crates/magic-eastmoney-rs/src/transport.rs#L233), [transport.rs:358](../../../crates/magic-eastmoney-rs/src/transport.rs#L358), [transport.rs:422](../../../crates/magic-eastmoney-rs/src/transport.rs#L422), [default timeout](../../../crates/magic-eastmoney-rs/src/lib.rs#L54).

BR-020 names exactly `finance`, `global`, `biz`, `stock`, `fund`, `futures`, `bond`, `bank`, `forex` and `hk` under `eastmoney.com`, and states that these are metadata links only. The integration note repeats this scope. BR-066 states that an explicitly selected GlobalNews Provider remains pinned and is not silently substituted after failure. Dropping the insurance row, returning fewer records, bypassing full-page validation or swapping to another source is not this candidate. [BR-020](../../business_rules.md#L494), [Eastmoney integration boundary](../eastmoney-web.md#L58), [BR-066](../../business_rules.md#L1258).

There is an older recorded insurance-host failure on 2026-09-25. It is historical supporting context, not a current success/failure rate or a replacement for today's exact receipt. [Earlier audit](../../evidence/2026-09-25-news-information-gap-audit.md#L35).

## Registry and Gate A candidate material

The HTTP registry currently records `magic-eastmoney-rs` as `legacy-direct`, direct dependencies `ring,ureq`, shared transport `false`, and a `reviewed-exception` whose reason covers the existing stack and exact Miaoxiang POST approval. That row does not independently approve new news metadata hosts. This candidate adds no HTTP/TLS dependency or request endpoint and must not be implemented by adding insurance to a network endpoint allowlist. [http-transports.tsv:6](../http-transports.tsv#L6), [exact news request allowlist](../../../crates/magic-eastmoney-rs/src/transport.rs#L422).

The admissions registry has six Eastmoney rows for Miaoxiang, bounded rankings and public flows, but no Eastmoney `GLOBAL_NEWS_ADMITTED` row. The code currently advertises `content_capabilities().global_news=true` directly. Therefore no news-host approval, news probe count, or new admission constant can be claimed from that table. A reviewer must explicitly choose the registry representation required for this amendment; if adding a registry-backed news constant/row is chosen, that is additional planned work, with actual counts and dates earned by the later prescribed probes. Do not invent a constant or copy another family's evidence. [admissions.tsv:47](../admissions.tsv#L47), [content_capabilities](../../../crates/magic-eastmoney-rs/src/lib.rs#L167), [BR-009](../../business_rules.md#L154).

Suggested approval text, offered for review only:

> Extend BR-020's exact Eastmoney finance-rolling metadata host set by `insurance.eastmoney.com` only. Accept that host only when supplied by a fully validated row in the exact first-page `财经` feed and only at `/a/<nonempty ASCII digits>.html`, with the existing strict HTTP(S) input syntax and scheme-only HTTPS normalization preserving host/path. The operation still fetches one fixed rolling page, performs complete-page validation before the limit, and never fetches articles or follows redirects. Keep source records, times, cardinality, errors and pinned Provider selection unchanged. Update the owning integration evidence and applicable registry record explicitly, then require deterministic regressions and bounded native live/serial admission evidence before any release claim.

Approval is required by Gate A because it changes the registered admitted source-metadata scope. Approval of this text would not itself deploy a build or replace the formal runtime. Runtime replacement and Mac acceptance must remain separately authorized and bound to their exact source/descriptor/binary identities. [Gates A–D](../../ENGINEERING_RULES.md), [repository authority boundary](../../../AGENTS.md).

## Bounded regression and verification plan: proposed, not executed

The existing suite already tests scheme normalization, exact first-party hosts, complete-page validation before truncation, duplicate identities and malformed URLs. Extend those seams rather than introducing article-network behavior. [global_news_regression_tests.rs:73](../../../crates/magic-eastmoney-rs/tests/internal/global_news_regression_tests.rs#L73), [existing host cases](../../../crates/magic-eastmoney-rs/tests/internal/global_news_regression_tests.rs#L98), [complete-page cases](../../../crates/magic-eastmoney-rs/tests/internal/global_news_regression_tests.rs#L126), [URL/duplicate cases](../../../crates/magic-eastmoney-rs/tests/internal/global_news_regression_tests.rs#L165).

| Proposed case | Required assertion |
| --- | --- |
| Exact insurance metadata URL under both input schemes | The same numeric ID and `https://insurance.eastmoney.com/a/<id>.html` are returned; the source hostname is preserved and not replaced with `finance.eastmoney.com` from the sampled Location |
| Observed insurance row in a small deterministic finance-page fixture | Preserve observed title/category/minute facts and numeric ID; assert RFC3339 time and original per-record minute evidence, absent summary/content and empty instruments; label the surrounding page fixture synthetic, not the original full response |
| Insurance row after the first two valid rows, request limit 2 | Complete page succeeds only after that third row is validated; output has exactly two selected records. Include insurance among selected rows in a separate case to assert its actual projected URL/evidence |
| Malformed insurance row after the output limit | Entire parse fails despite two earlier valid rows: mismatch title, invalid minute timestamp, wrong category, malformed group or unsafe URL is not skipped |
| Same numeric ID under finance and insurance hosts | Entire page fails with duplicate-ID evidence, even though the URL strings differ |
| Exact negative hostname/authority cases | Reject `insurance.eastmoney.com.example`, `evil.insurance.eastmoney.com`, `insurance.eastmoney.com.`, explicit `:443`/`:80`, userinfo, IP addresses, unrelated Eastmoney channels and wildcard-style suffix acceptance |
| Exact negative path/suffix/scheme cases | Reject `/b/123.html`, `/a/.html`, `/a/not-digits.html`, `/a/１２３.html`, `/a/%31.html`, nested/traversal paths, `.html/extra`, query, fragment, relative/protocol-relative URLs and non-HTTP(S) schemes |
| Recording transport during one `global_news` call | Exactly one bounded HTML GET of `https://roll.eastmoney.com/finance.html`; no insurance or finance article GET, HEAD, redirect resolution, retry or alternate Provider request |
| Existing transport boundary regressions | Exact page only, no page 2/query/fragment/lookalike, UTF-8 HTML requirement, 2 MiB cap, timeout and redirect rejection remain intact; article hosts remain invalid as the news-page fetch endpoint |
| Selected-Provider/service evidence | A fully evidenced Eastmoney batch passes existing v2 evidence checks; a malformed/unadmitted row retains an explicit failure with no records and does not cause source substitution |

Direct helper cases can use the observed ID `202610083889432650`; tests needing observation-time comparisons must use a fixed injected observation after the publication label or explicitly synthetic past labels. Never move a source time forward/backward to manufacture a live success. No date-prefix interpretation or fixed article-ID length is established by this one sample. [Existing ID validation](../../../crates/magic-eastmoney-rs/src/news.rs#L309), [BR-054](../../business_rules.md#L889).

After an approved implementation, the narrow native command `cargo test -p magic-eastmoney-rs --locked --offline global_news` can exercise the current named regressions. The complete required Gate C checks still include formatting, relevant tests, Clippy, compliance and documentation checks. The documented Eastmoney news/live/load entry points can supply fresh bounded evidence; the new host must actually appear in evidence, since a successful feed containing only previously admitted hosts does not prove the added scope. Retain every failure and distinguish a source-window observation from full historical coverage. This research ran no Rust tests, rebuilt no binary, and executed no additional service RPC. [Gate C](../../ENGINEERING_RULES.md#L6), [documented live/load probes](../eastmoney-web.md#L220), [BR-009](../../business_rules.md#L154), [BR-021](../../business_rules.md#L175).

## Evidence strength and open items

| Evidence class | What it establishes | What remains outside it |
| --- | --- | --- |
| First-party feed E1 | Exact insurance-host numeric link was supplied in a source `财经` row; source title/minute/category and channel navigation were observed | Original 11:54 URI/body, all future feed layouts or every insurance path |
| First-party responses E2–E4 | This sample's HTTP→HTTPS and insurance→finance Location headers; HTTPS requests completed with default validation; finance page metadata matched | Permanent redirect behavior, standalone 200 insurance content, exhaustive canonical rules, machine-use licence or authorship |
| Exact original RPC receipt | Pinned limit-2 native request failed with the supplied host-admission reason and no response | Upstream article identity/body, Mac consumer acceptance or downstream delivery |
| Current repository source/rules | Why complete-page rejection occurs; exact metadata/network boundaries and existing test seams | An approved scope amendment, passing new tests or new deployment |
| Inference/proposal | Existing source category/path and first-party linkage support considering the one exact host under the same metadata-only contract | Admission; the reviewer must accept the proposed identity semantics despite the sampled redirect |

Open items before release: approved Gate A scope and registry recording choice; deterministic implementation/regressions; replayable evidence if required by the acceptance owner; prescribed native live/serial observations proving the added host; complete Gate C checks; separately authorized deployment and Mac acceptance. If the reviewer requires the original failing upstream URI, it is currently unavailable from the safe receipt and must be obtained from a future bounded source capture or an existing retained raw response, not reconstructed from this later sample. [Admission rules](../../business_rules.md#L154), [Gates](../../ENGINEERING_RULES.md); original receipt at the exact UNC path cited above, lines 1–25.

Research limitations/failures preserved: browser-tool opens of the sampled insurance article and the guessed `https://about.eastmoney.com/home/law.html` were unavailable; the rolling-page legal-link click was also unavailable. No authorization or licence claim is based on them. A search for an assumed `tools/compliance/check_integration_admissions.py` reported that the file does not exist; subsequent file discovery found `check_admissions.py`. CodeGraph was consulted first; some test-file/symbol queries returned unrelated or truncated output, so the actual unshown regression file and relevant rule line numbers were then read directly. None of these failures was counted as admission evidence.

## Local source identity recorded during research

These hashes were read from current files at HEAD `841e4ae7a9df62be0c4536fa9009c66089d282bf`; they identify the source/rule evidence used, not a modified candidate build.

| Repository file | SHA-256 |
| --- | --- |
| `crates/magic-eastmoney-rs/src/news.rs` | `62ade7a3c602c5f6be1326a7cba00bb1019ff1c000330eb597e016a3ce015ef5` |
| `crates/magic-eastmoney-rs/src/transport.rs` | `68d546cc07def2fb4a20f69b541eb767244759e3d7b1ad75f6ce3b150e1fd038` |
| `docs/business_rules.md` | `2244becaa38a43c94883e58f4a8c4c64d56aa89a17c642688dd2bfb34ee6fe7a` |
| `docs/integrations/admissions.tsv` | `c784fd2f7f705057524c67c3c0e415cc435755e8537c599117db73da85343cb8` |
| `docs/integrations/http-transports.tsv` | `0f9b285f929cb5dd2e429e7d26f554984c310dd60296a181c80a565960c708b2` |

Research-document verification: the repository `tools/docs/check_links.sh` ran under Git Bash and exited 0; a separate note-scoped check verified 45 local link targets and referenced line bounds with no missing/out-of-range target. The five source/rule/registry hashes above remained unchanged, and `git diff --exit-code` for those files exited 0. Final status showed this new research note plus the two pre-existing untracked 2026-10-07 upstream-probe directories; this research did not alter those directories. These checks verify the research artifact and unchanged scoped inputs, not Provider admission, runtime health or a bug fix.
