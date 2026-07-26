# Findings: WallstreetCN RSS News Provider

## Source Evidence

- Exact feed: `https://dedicated.wallstreetcn.com/rss.xml`.
- 2026-07-26 bounded probe: HTTP 200, 359627 bytes, 54 items.
- Observed media type: `text/html; charset=UTF-8`.
- Required channel identity: title `华尔街见闻`, link
  `https://wallstreetcn.com`, language `zh-hans`.
- Required item fields: title, `/articles/{decimal_id}` link, source
  `华尔街见闻`, and RFC 2822 `pubDate`.
- RSS descriptions contain article content and are forbidden output.
- The admitted release probe returned 20 strict metadata-only rows and the
  two-request load probe returned 10 rows twice with a 7.529-second total.
- The current bounded feed included article `3777926`, titled
  `韩国绑定美国AI产业链！三星、SK海力士与英伟达、博通等签9500亿美元芯片大单`;
  a case-sensitive local `9500亿美元` match passed.
- Production evidence therefore admits `global_news=true`. Instrument news,
  descriptions, bodies, article-page fetching, and inferred instruments
  remain unsupported.

## Architecture

- The existing Yonhap implementation proves the locked `quick-xml 0.41.0`,
  `time 0.3.54`, and `ureq 2.12.1` stack can implement bounded RSS parsing,
  explicit source time, clone-shared pacing, and typed live admission.
- WallstreetCN remains a separate crate; no cross-Provider dependency or
  Router production dependency is permitted.
- The currently observed `text/html` media type is accepted only for the exact
  RSS URL and only when the complete document satisfies the strict RSS
  contract.
- Descriptions and extension subtrees are structurally consumed but their text
  is never accumulated, mapped, or serialized.
- The full source document is validated before the requested 1–50 row limit is
  applied, so malformed or duplicate later rows cannot be hidden by
  truncation.
- Cloned clients share one request gate and hold it through the transport call,
  preventing concurrent clones from bypassing the one-second start interval.

## Rights Boundary

- The public RSS endpoint is not treated as permission to redistribute article
  content.
- Descriptions, bodies, excerpts, images, article pages, hidden APIs, login
  state, storage, caching, indexing, and inferred instruments are excluded.

## Release Evidence

- Strict coverage passed at 80.31% overall and 95.97% for critical modules.
- Formatting, locked offline check/test/Clippy, warning-free Rustdoc,
  documentation tests, documentation links, compliance, and the complete
  isolated release preflight all passed.
- The standalone crate has no downstream path dependency. Router remains
  Provider-neutral and does not depend on the WallstreetCN crate.
- Cargo reports a non-fatal workspace warning because several Providers use
  the conventional example names `live_probe` and `load_probe`; the release
  package script already emits Provider-qualified artifact names.

## Independent Review

- The first final review found no Critical issue and returned “with fixes”
  because ignored XML content did not receive complete XML 1.0 validation and
  declarations were not fully constrained.
- Adversarial tests proved that literal forbidden characters inside ignored
  content, forbidden character references in ignored attributes, invalid
  comments, malformed declarations, and duplicate declarations could pass.
- The parser now validates the complete UTF-8 document against the XML 1.0
  character repertoire, revalidates every decoded attribute, enables comment
  checks, and permits at most one ordered `version="1.0"` declaration with
  optional UTF-8 encoding and valid standalone state.
- Ignored Text and CDATA are no longer decoded or normalized. Their literal
  characters are covered by the document scan, references remain validated by
  the existing entity path, and no content is accumulated or emitted.
- Follow-up review found that numeric references to `U+FFFE` / `U+FFFF`
  bypassed the literal-character scan and that declaration attribute
  normalization could turn entity references into accepted fixed values.
- Numeric references now use the same XML 1.0 repertoire predicate as literal
  characters, while declaration values are compared as raw UTF-8 and cannot
  contain entity spellings that merely normalize to an accepted value.
  Adversarial tests cover both recognized and ignored content, plus encoded
  declaration values; positive declaration forms remain covered.
- Final live, two-request load, focused tests, strict coverage, and the
  complete isolated release preflight all passed after the second remediation.
