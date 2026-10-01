# Remaining official-publication admission

## Gate A decision

The user requested admission on 2026-10-01 after the first four publishers were
verified. Continue the same bounded native-library slice: each remaining source
must independently satisfy contract tests, two live listing/original rounds and
three serial list calls before its admission constant changes. No source may be
enabled solely because the user requested admission.

## Verified CSRC date contract

The CSRC original pages visibly state `日期：YYYY-MM-DD 来源：…` in
`.main > .content > .info > p.fl`, and the visible title is
`.main > .content > h2`. Preserve that entire visible date line as the label,
parse only its anchored numeric date, and return `Date` precision. The list API's
`publishedTimeStr` remains its separate source label with its original precision.
Original `PubDate` is not eligible: observed pages use the same value for
`others` explicitly described as page generation time. List and article dates
must agree in live admission; their time precision need not agree.

The current complete 18-row `_isAgg=true` list also declares originals from the
same host's leadership channel `c106311`. Its public original has the same
verified title/date/body layout. Allow only the two explicit original paths
`/csrc/c100028/cID/content.shtml` and `/csrc/c106311/cID/content.shtml`; do not
generalize to arbitrary CSRC channels or discard the leadership rows. The shared
HTML policy adds only `/csrc/c106311/` to this source's existing prefix. Two live
rounds must each sample the first two originals plus one leadership original.
The list must declare numeric `page=1`, `rows=18`, its exact source channel ID,
and string `relateSubChannels="true"`. Its result count must equal
`min(total, 18)`; a shorter non-empty page is not admitted as complete.

Add a closed `PublicationLabelOrigin` to native references and originals so a
consumer can distinguish HTML list rows, API list fields, article metadata and
visible article date lines. This is an additive native contract change within
the unreleased first slice, with no `GlobalNews` projection or inferred zone.
An absent, repeated or malformed visible date/title/body is a protocol error;
there is no fallback to ambiguous metadata. Other sources retain their existing
metadata rules. Hashes continue to bind the entire responses, including ignored
generation metadata.

## Other profiles and transport

Investigate MOF, MIIT and NEA using their own official HTTPS HTML/JS/API only.
Any newly proved narrower fixed column must be recorded by its exact name and
endpoint before its implementation. Do not silently change HTTP references,
skip unexpected publication rows or claim whole-site coverage. If a profile
cannot satisfy this contract, preserve independent blocked admission with fresh
evidence. GACC must work with the existing shared Rust transport before template
admission; no certificate exception or trust-store mutation is authorized.

No local HTTP/TLS dependency, redirect, body-size or timeout policy changes are
planned. Any fixed endpoint change remains in the shared transport mode and is
recorded in the profile evidence and HTTP registry's documented scope.

## Verified narrower source profiles (Gate A refinement)

- MOF is fixed to the comprehensive department's **政策发布** at
  `https://zhs.mof.gov.cn/zhengcefabu/`. Replace the old main-host profile with this
  one host, not a cross-host fallback. Its ten `ul.liBox > li` rows use relative
  originals, with `/zhengcefabu/YYYYMM/tYYYYMMDD_ID.htm` paths. This proves only
  that department's published policies (including lottery funds and mining
  rights), not the main financial-news feed or whole ministry. Visible
  `h2.title_con`, visible `.docreltime > span` labelled
  `发布日期：YYYY年MM月DD日` and `.my_doccontent > .TRS_Editor` form the original
  contract. Preserve the visible date with `VisibleArticleDate` origin and
  date-only precision; metadata `PubDate` must not silently raise its precision.
- MIIT is fixed to **部领导活动**,
  `https://www.miit.gov.cn/xwfb/bldhd/index.html`. Its exact website unit request
  uses `pageId=d3e2bede1bc045e2875fc7161c01db7d`, `tagId=右侧内容`,
  `parseType=buildstatic`, `webId=8d828e408d90447786ddbe128d495e9e`,
  `tplSetId=209741b2109044b5b7695700b2bec37e`, `pageType=column`, `editType=null`.
  All 24 current-page originals are root-relative paths in `/xwfb/bldhd/art/`.
  Validate the successful JSON envelope, full HTML rows and page-one pagination
  before applying the caller's limit. Rows are
  `div[id=右侧内容] > div.page-content > ul > li.cf`, with primary `a.fl` and
  `span.fr` date. Original title is `h1#con_title`, body `div#con_con.ccontent`.
  `.cinfo > span#con_time` visibly states `发布时间：YYYY-MM-DD HH:MM`;
  preserve that minute label with `VisibleArticleDate` origin. Two verified
  originals' metadata `PubDate` disagrees with the visible publication label;
  it must not override the visible label or supply a newer day. Visible dates
  agree with the listing rows. This is not the latest-policy feed.
- NEA is fixed to the news-center's **top ten bureau-work updates** at
  `https://www.nea.gov.cn/xwzx/index.htm`, two five-row lists inside
  `.xwzx-page01 .xwzx-yw-right .xwzx-yw-box` with datasource identifiers
  `91fb3999d1964141b668e4a4cef4ed98` and `64763711c745408cb6f3bc0895f37649`.
  Both containers and all ten rows must validate. Other source containers on
  the news-center page are outside this explicitly bounded scope. Original
  `.article-title > .titles`, `.article-title > span.times` and `#detailContent`
  prove title, a visible `发布时间：YYYY-MM-DD` date, and body. Preserve visible
  date-only precision rather than choosing between its two metadata date
  precisions. No page scripts, images, external media links or more-page
  dependencies are fetched.

The HTTP transport registry retains the existing `shared` entry: these are
fixed profile replacements using the already registered shared transport, with
no dependency or transport-policy widening. Exact hosts, paths and unit request
values are also recorded in the admission registry's integration evidence.

## Gates B through D

Test visible-date authority independently of generation metadata, malformed and
duplicate labels, explicit origin evidence, exact URL boundaries and validation
of all source rows before caller limits. Run native live probes on implemented
profiles, preserve failures, then formatting, relevant/workspace tests, Clippy,
documentation, compliance, link and dependency checks. Update admission evidence
and registry only for capabilities actually proved. Service registration,
archival storage, automatic scheduling and event extraction remain deferred.
