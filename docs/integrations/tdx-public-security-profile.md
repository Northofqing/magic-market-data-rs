# TDX public security-profile admission

## Exact scope

`magic-tdx-rs::TdxSecurityProfileProvider` admits only one to eight unique
Shanghai or Shenzhen equities. It combines two existing public TDX protocol
families on the same verified endpoint:

- normalized security metadata supplies exact identity, source name and the
  optional finance-backed listing date;
- F10 supplies the exact unique `公司概况` section.

Every non-empty source line in that section becomes an ordered `ProfileFact`.
Whitespace is normalized, but labels and text are not interpreted as industry,
share counts or another semantic field. `industry`, `total_shares` and
`floating_shares` therefore remain `None`. Empty/duplicate requests, Beijing or
non-equity identities, missing/duplicate sections, missing names, empty content,
more than 256 facts or one failed instrument reject the complete batch.

The TDX F10 wire content length is `u16`; the Provider additionally limits a
request to eight instruments. F10 supplies no source timestamp, so all record
and batch `source_at` values remain absent. Observation time and batch identity
are local evidence only.

## Deterministic evidence

Unit tests cover bounded/unique/equity-only validation, exact category identity,
empty and over-limit sections, ordered facts, unavailable semantic fields,
provider/batch evidence and fail-before-I/O invalid requests.

## Live evidence — 2026-08-14

The production Rust client used `180.153.18.170:7709`, a 10-second socket
timeout and exact `600396.SH`. Before admission, two independent bounded
diagnostic runs returned the same source identity:

```text
live 1: name=华电辽能 listed_on=2001-03-28 facts=132 elapsed_ms=1942
live 2: name=华电辽能 listed_on=2001-03-28 facts=132 elapsed_ms=1961
```

A serial three-request run returned 3/3 strict batches, each with 132 facts and
distinct batch evidence, in 4702 ms total. All five batches used
`ProviderId::Tdx`, were complete and had `source_at=None`.

After promoting `SECURITY_PROFILES_ADMITTED=true`, the same formal
`SecurityProfiles` trait was run twice and then three times serially again. The
formal runs retained the same name, listing date, 132 facts, strict quality and
absent source time. Exact runtime output is intentionally summarized rather
than storing the full F10 text in Git.

```text
cargo run -p magic-tdx-rs --example security_profile_probe --release --locked --offline
MAGIC_TDX_SECURITY_PROFILE_REQUESTS=3 \
  cargo run -p magic-tdx-rs --example security_profile_probe --release --locked --offline
```

This admission does not cover Beijing, arbitrary F10 sections, inferred
fundamentals, realtime freshness or a redistribution right beyond the
deployment operator's use of the public protocol.

## Release gRPC verification

The release `magic-market-grpc-server` was restarted with the same production
composition and exercised through the deployed mTLS + Bearer endpoint. A
`SecurityProfiles` request using schema
`magic.market.security_profiles.request`, `preferred_provider=Tdx` and exact
`600396.SH` returned `ADMISSION_STATE_ADMITTED`, `selectedProvider=Tdx`, one
complete `magic.market.security_profile` record and a non-empty ordered fact
set. The same capability snapshot reported 54 operations, with 46 operations
having an admitted handler and the remaining eight preserving their exact
fail-before-I/O blocker.
