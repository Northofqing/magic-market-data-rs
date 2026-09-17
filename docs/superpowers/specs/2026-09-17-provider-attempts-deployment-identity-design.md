# Provider attempts and deployment identity contract

Date: 2026-09-17

## Problem

The public protobuf exposed `ProviderAttemptDetail`, but the documentation did
not publish its complete value vocabulary or legal boolean combinations. The
gRPC boundary also used `take(16)`, which could silently turn an oversized
internal route trace into an apparently complete public trace.

The bundle identified source files but did not carry the exact build identity
returned by the deployed endpoint. A client could compare source commits but
could not establish the expected descriptor-set and executable digests before
admitting a connection.

## Gate A decision

Keep the protobuf wire unchanged and close the existing string fields in the
public documentation and service constructor. Provider identity is the exact
case-sensitive identity advertised by the endpoint's capability snapshot;
outcome, reason, retryable and terminal use the published matrix. Ordinals are
one-based and contiguous. A trace longer than 16 entries fails as safe
`INTERNAL/internal` with no partial trace.

Extend `bundle-metadata.json` with an optional `deployment_build_identity`.
When supplied, all identity fields are mandatory and validated. Its contract
digest is SHA-256 over the raw compiled `FileDescriptorSet` bytes exported by
`magic_market_grpc_contracts::v1::FILE_DESCRIPTOR_SET`; its binary digest is
SHA-256 over the exact deployed server executable. A null identity is explicit
missing evidence, never an instruction to trust the first observed Health.

## Alternatives rejected

- Silently taking the first 16 attempts loses terminal route evidence.
- Treating arbitrary lowercase strings as safe reason codes lets producer and
  consumer control logic drift independently.
- Hashing `market.proto` text as the contract identity does not match the
  deployed Health contract hash, which covers compiled descriptor bytes.
- Deriving expected build identity from the first endpoint response makes a
  man-in-the-middle or wrong deployment self-authorizing.

## Trade-offs

The wire remains compatible and old clients can ignore new metadata fields.
Strict clients gain deterministic recovery and endpoint admission. Adding a
new reason now requires an explicit versioned documentation and implementation
change. Release packaging must receive the actual deployed descriptor and
binary digests to publish a non-null identity.

## Verification seam

- Unknown or conflicting attempt states fail construction.
- Seventeen attempts produce no truncated public array.
- Sixteen or fewer attempts retain one-based contiguous order.
- Bundle generation rejects partial or malformed deployment identities.
- A generated bundle manifest covers the metadata containing the expected
  deployment identity.
