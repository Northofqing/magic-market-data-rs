# Upstream source

`magic-tdx-rs` directly incorporates the pure-Rust modules from
[`jiangtaovan/tdxrs`](https://github.com/jiangtaovan/tdxrs), pinned at commit
`18b05ffc9d8a257b5ba5add8a2d1ab038261747d` (version 0.6.7, MIT).

Imported areas are protocol parsing, readers, networking clients, block/fund
services, and F10/profile parsing. The Python module tree and PyO3 registration
were excluded. Local hardening and workspace integration are documented in the
repository history; the upstream license notice is preserved in
`LICENSES/tdxrs-MIT.txt`.

All other Providers in this workspace are independent local implementations
against their documented public or official network contracts. In particular,
the market-discovery/global/calendar work does not copy implementation code from
qshare: Eastmoney, CNInfo, Sina, Jin10, State Council and CFFEX adapters use
their own typed contracts, bounded transports, deterministic fixtures and
provenance validation.

The Yonhap adapter is an independent local implementation against the official
simplified-Chinese RSS directory at <https://cn.yna.co.kr/channel/rss> and the
Chinese terms at <https://cn.yna.co.kr/aboutus/copyright>, reviewed on
2026-07-25. It reads bounded RSS metadata only, does not copy upstream source
code, does not fetch article pages, and does not store or redistribute article
bodies.

The WallstreetCN adapter is an independent local implementation against the
public first-party feed at <https://dedicated.wallstreetcn.com/rss.xml>, the
publisher website at <https://wallstreetcn.com/>, and the first-party user
agreement at <https://wallstreetcn.com/articles/3522782>, reviewed on
2026-07-26. No WallstreetCN source code, private API, login state, cookie,
description, or article body is included. The adapter reads bounded RSS
metadata only and does not fetch article pages.
