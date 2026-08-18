# Final four capability admission design

## Goal

Close the four remaining gRPC capability gaps without fabricating unavailable
Level-2 fields, provider timestamps, universe coverage, or transport security.
The production contracts remain read-only and fail closed outside their exact
evidence-backed scopes.

## Auctions

`Auctions` v1 is an opening-auction observation contract, not the complete Core
Level-2 `AuctionSnapshot` conformance contract. One fixed Miaoxiang query must
return opening-auction matched quantity in shares and matched amount in CNY for
one exact A-share instrument and date in one HTTPS response. Those two fields,
instrument, source name, source date and one response identity are required.
Matched price, previous close, change, unmatched bid/ask queues and volume ratio
remain nullable. Their absence does not make the narrower observation partial.
The complete Level-2 Core contract remains separately unadmitted.

## Market breadth

One fixed Miaoxiang query must return, in one response identity and for one exact
date and all-A-share universe, listed security count, up, down, flat, limit-up
and limit-down counts. `valid = up + down + flat`, listed count must be at least
valid, limit-up/down must be subsets, and coverage is exactly valid/listed.
Because all fields are one date-level provider response rather than a
multi-request quote composition, acquisition skew is structurally zero. Because
the response has no provider field-level instant, `maximum_source_skew_millis`
remains `null` rather than fabricating a source-time value. Local
observation time remains distinct from the provider source date.

## Market rankings

The admitted gRPC scope is a bounded, single-response provider ranking snapshot,
not a full-universe multi-page copy. It accepts only the two source-proven metrics
and a positive limit at most the fixed source page size. Every returned row must
have exact security identity and name, non-null metric and unit, continuous
source rank, source date/time, provider-declared universe total and the common
response evidence. The HTTP response is the atomic acquisition boundary;
different per-security source update times remain visible and are not relabelled
as simultaneous. Full-market pagination remains a separate false capability.

## Futures delivery

The formal path performs no runtime HTTP request. It serves only a checked-in,
versioned 2026 CFFEX equity-index-futures schedule whose twelve delivery dates,
four products and cash-delivery method are tied to official product rules,
official holiday adjustments and observed delivery notices. Requests outside
2026 fail before I/O. The existing exact-path plaintext CFFEX notice reader stays
diagnostic and can never populate the formal response. A later year requires a
new reviewed schedule revision and evidence update.

## Admission and runtime

The Miaoxiang families are repository-admitted only for the fixed templates and
schemas above; runtime availability still requires `EASTMONEY_API_KEY`. Clients
do not set `allow_unadmitted` for formal calls. No new host, path, redirect,
timeout or response-size permission is introduced. Formal futures delivery has
no network transport. All four scopes require deterministic tests, two bounded
live observations and three serial observations before the registry changes are
released.
