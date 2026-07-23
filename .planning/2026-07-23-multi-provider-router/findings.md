# Findings

- Commit `ce7f1c6` now has a verified release package under
  `target/dist/ce7f1c623b7bfc3764f95860e189acbc00238d3f`; all SHA-256 entries
  pass.
- Core already provides normalized requests and records for Quote, bars,
  minute data, trades, money flow, books, auctions and security metadata.
- `DataBatch` exposes records, provenance and quality, but normalized record
  types do not yet share a common provider/batch evidence trait.
- A generic route can remain independent of concrete providers by accepting an
  error-classification closure when adapting an existing Core provider trait.
- TDX normalized Quote batches lack verified source time; Tencent Quote batches
  carry it. A live route requiring complete quality and batch source time
  therefore exercises a real TDX rejection followed by Tencent selection.
- The release script currently packages four probes and the compliance script
  asserts an exact four-member workspace, so both must change with the router.
- EMQuant activation refreshed `target/emquant/runtime/userInfo` at
  `2026-07-23 14:22:04 +0800`, proving that the SMS/device activation completed.
  A clean SDK process still returns `10001003/EQERR_NO_ACCESS` before every
  query. The vendor header confirms that `start(nullptr, ...)` is the supported
  `userInfo` login path after SDK 2.0, so the remaining blocker is server-side
  API entitlement propagation; the router must not pretend that permission is
  available.
- A minimal official ABI login with `ForceLogin=1` also returns `10001003`,
  ruling out a stale concurrent API session. Choice must enable or propagate
  the API product entitlement separately from device/SMS activation.
