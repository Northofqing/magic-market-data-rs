# magic-market-core

Provider-neutral checked market-data contracts.

The core distinguishes Shanghai, Shenzhen, and Beijing exchanges and exposes
source-evidenced contracts for quotes, bars, trades, five-level books, money
flow, auctions, and security metadata. Metadata keeps board, ST status, listing
date, and the price-limit rule/version independently optional: a provider may
return fields it can prove without inventing the rest.
