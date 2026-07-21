# Upstream source

`magic-tdx-rs` directly incorporates the pure-Rust modules from
[`jiangtaovan/tdxrs`](https://github.com/jiangtaovan/tdxrs), pinned at commit
`18b05ffc9d8a257b5ba5add8a2d1ab038261747d` (version 0.6.7, MIT).

Imported areas are protocol parsing, readers, networking clients, block/fund
services, and F10/profile parsing. The Python module tree and PyO3 registration
were excluded. Local hardening and workspace integration are documented in the
repository history; the upstream license notice is preserved in
`LICENSES/tdxrs-MIT.txt`.
