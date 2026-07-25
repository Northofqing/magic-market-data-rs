# Eastmoney Global Latest-News Design

## Goal

Admit a read-only Eastmoney `NewsProvider::global_news` implementation for the
latest public finance articles. The output is a global news stream: text
mentions are not promoted into stock identities, and the existing
`instrument_news` boundary remains explicitly unsupported.

## Source decision

Three official Eastmoney surfaces were considered:

1. `https://roll.eastmoney.com/finance.html` is a server-rendered,
   minute-updated chronological finance list with source timestamps, titles and
   canonical article links. This is the selected source because its contract is
   directly observable, credential-free and does not need JavaScript signing.
2. `https://kuaixun.eastmoney.com/` refreshes faster but its application API is
   more volatile and may require undocumented client parameters. It is not
   admitted in this change.
3. `https://finance.eastmoney.com/` is a curated homepage rather than a single
   chronological stream; section overlap would create ambiguous ordering and
   duplicate semantics. It remains outside this contract.

The user approved a standalone Eastmoney global news stream and delegated
project-level design confirmation. The implementation therefore uses option 1.

## Contract

`EastmoneyClient::global_news(limit)` accepts `1..=20`. It requests only the
first official finance rolling page and validates the complete `#artList`
before applying the caller limit.

Each admitted row maps to `NewsItem`:

- `item_id`: the digit-only article identity extracted from the exact canonical
  `/a/<id>.html` path;
- `title`: decoded, non-empty HTML title text;
- `summary` and `content`: `None`, because the list page supplies neither;
- `publisher`: `东方财富网`;
- `canonical_url`: normalized HTTPS URL on `finance.eastmoney.com`;
- `published_at`: the exact `YYYY-MM-DD HH:MM` list timestamp;
- `instruments`: empty; title mentions are not structured source identities;
- `topics`: one source family label, `财经`;
- `language`: `zh-CN`;
- `evidence`: `ProviderId::Eastmoney`, row timestamp, observation time and
  operation batch ID.

The batch `source_at` is the newest row timestamp. Rows must be newest-first,
article IDs and URLs must be unique, and the source page must contain at least
the requested number of records.

## Transport and failures

Production adds an HTML-specific transport method rather than weakening the
existing JSON/JSONP media-type gate. It accepts only HTTPS
`roll.eastmoney.com/finance.html`, rejects redirects, requires
`text/html; charset=utf-8`, caps the response at 2 MiB and shares the existing
one-second request gate across client clones.

Missing `#artList`, malformed list structure, invalid calendar/clock text,
wrong category, non-official or noncanonical article URLs, duplicate IDs,
unsorted rows, truncated response, empty result and insufficient cardinality
are explicit errors. No retry, cookie, account, article-body crawl or fallback
HTML selector is hidden inside the Provider.

## Capability, routing and operations

`ContentCapabilities::global_news` becomes true for Eastmoney;
`instrument_news` remains false. The existing provider-neutral
`global_news_source` router adapter is reused without a production dependency
on the Eastmoney crate.

The Eastmoney live probe validates and prints the new global stream by default.
The `news` load operation switches from an unadmitted keyword diagnostic to the
admitted global-news operation. Keyword-only instrument search remains a
separate explicit unsupported boundary and is not used by the probes.

## Verification

Deterministic tests cover:

- exact URL, limit and capability;
- complete mapping and HTTPS normalization;
- newest-first ordering, unique identities and exact evidence;
- invalid timestamp, category, host/path, duplicate, missing container,
  insufficient rows and HTML content-type failures;
- router acceptance under `ProviderId::Eastmoney`.

The live probe must return non-empty current records with source timestamps and
canonical Eastmoney article URLs. Repository formatting, workspace tests,
strict Clippy, rustdoc, doctest, documentation links, compliance and diff
checks remain mandatory.
