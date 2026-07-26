# Findings

- Current per-instrument entry identity is `<code>:<date>` and therefore
  rejects legitimate multiple reasons on one security/date.
- Real Eastmoney discovery rows contain `TRADE_ID`; 2026-07-22 examples
  include 002396 and 600396 with several distinct reasons.
- Real Eastmoney seat rows also contain `TRADE_ID`.
- Querying seats by security/date alone interleaves independent reason groups;
  exact `TRADE_ID` filtering is required before enforcing 5+5.
- Existing Router already validates individual entry and seat batches, but
  there is no atomic disclosure record or whole-market request.
- The broad discovery report also returns convertible bonds
  (`SECURITY_TYPE_CODE=060`). A-share equity rows carry the verified source
  type `058001001`, so the market operation must filter that explicit field.
- Probe admission requires record and batch `source_at` to match exactly; a
  raw source midnight timestamp cannot be mixed with an ISO-date provenance.
