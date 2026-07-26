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

- Strict coverage passed at 80.28% overall and 95.97% for critical modules.
- Formatting, locked offline check/test/Clippy, warning-free Rustdoc,
  documentation tests, documentation links, compliance, and the complete
  isolated release preflight all passed.
- The standalone crate has no downstream path dependency. Router remains
  Provider-neutral and does not depend on the WallstreetCN crate.
- Cargo reports a non-fatal workspace warning because several Providers use
  the conventional example names `live_probe` and `load_probe`; the release
  package script already emits Provider-qualified artifact names.
