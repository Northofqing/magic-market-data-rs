# Hithink metadata date compatibility

## Gate A scope and evidence

The user's existing repair request covers this local SDK repair and public Mac
handoff. The supplied WG07 caller contract is now available, but no qualified
native source profile has been established. This slice repairs the connected
`SecurityMetadata` decoder; it does not implement or advertise WG07 production
support. No listener, provider call, credential access, deployment, push,
admission promotion, dependency or HTTP policy change is authorized here.

The pinned official [ticker-search document](https://github.com/HiThink-Tech/Financial-API/blob/3bca7805a4127ece8d81961917e740d2effac6ec/docs/api/meta/tickers-search.md)
defines four nullable strings in `yyyy-MM-dd` format: `list_date` (listing),
`end_date` (contract expiry), `last_trade_date` and `last_delivery_date`.
Its current-snapshot loading timestamp does not establish historical fact
availability. These are documentation facts, not a live response acquisition.
The response table does not promise omission; accepting omission is our explicit
backward-compatibility policy for the previously supported six-field shape.

## Behavior and seams

The existing `SecurityMetadataProvider::security_metadata` / public probe and
the existing composition `OperationRegistry::execute` handler are the test
seams. They remain connected to the same bounded, injectable external HTTP
transport. In accordance with the user's routine-work autonomy instruction,
no extra confirmation is requested for those existing seams.

Only these four fields become known private DTO fields. Preserve their
Absent, Null or exact Value state until normalization; validate every returned
item's non-null dates as canonical Gregorian dates before filtering the exact
identity. Invalid known dates/types and all unknown fields reject atomically.
Do not invent date ordering or reinterpret expiry as delisting.

The private native fields are not exported by this slice. The existing Core
and RPC projection remains unchanged: `listed_on`, board, ST and price-limit
rules are absent, status is `Unavailable`, exact identity/name and current
batch provenance are retained. No public native-date outcome or new schema is
introduced. A complete identity batch is not a complete lifecycle certificate.

## Gates B through D

First reproduce rejection of the documented four-field shape through the
public provider seam with offline HTTP fixtures. Implement the minimal known
field decoder, then test null/absence/value compatibility, strict date/type
rejection (including malformed nonmatching rows), unknown fields and atomic
multi-instrument failure. Test the existing composition handler's version-1
projection and retain historical-bar version-1/version-2 regressions.

Run locked relevant tests, formatting, Clippy, documentation and compliance
checks. Deliver actual source commit and bounded public evidence with explicit
test and runtime limits. Do not relabel an older server executable as this
source, increase codec limits or expose a synthetic source profile. All WG07
native availability, correction, exhaustive session and same-source complete
lifecycle requirements remain independent and unresolved.
