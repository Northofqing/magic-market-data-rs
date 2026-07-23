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
- EMQuant entitlement remains an external `10001003/EQERR_NO_ACCESS` blocker;
  the router must not pretend that permission is available.
