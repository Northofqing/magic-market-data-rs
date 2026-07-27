# Rankings, breadth, consensus and target-price evidence — 2026-07-27

This record separates deterministic contract evidence from live public-web
admission. A failed or interrupted full-market source is not converted into an
empty result and its capability remains false.

## Deterministic contracts

The following focused suites passed with locked, offline dependency
resolution:

```bash
cargo test -p magic-market-core --test market_rankings --test target_price --test research --locked --offline
cargo test -p magic-market-analysis --test breadth --locked --offline
cargo test -p magic-eastmoney-rs market_rankings::tests --locked --offline
cargo test -p magic-eastmoney-rs --test market_rankings --test target_price --locked --offline
cargo test -p magic-eastmoney-rs reports::tests --locked --offline
cargo test -p magic-tdx-rs --test concept_hits --locked --offline
cargo test -p magic-tdx-rs service::blocks::tests --locked --offline
cargo test -p magic-ths-rs consensus --locked --offline
cargo test -p magic-market-router --example consensus_live --locked --offline
cargo test -p magic-market-router --test target_price_routing --locked --offline
```

The checked contracts establish:

- ranking code and name, contiguous rank, metric/unit identity, complete
  pagination, declared-universe coverage, Shanghai/Shenzhen/Beijing presence,
  source date/session, and one common source time with zero skew;
- volume-ratio and main-net-inflow admission are separate
  `MarketRankingCapabilities`; the legacy aggregate is true only when both are
  admitted;
- a ranking page retries at most three typed transport failures through the
  existing shared request gate. A later success is accepted, the third
  transport error is retained, and protocol errors are not retried;
- a source `f124=0` row fails the entire ranking instead of being silently
  omitted;
- market breadth uses an explicitly versioned and evidenced security-master
  universe, one atomic quote batch, and complete typed upper/lower limit-pool
  inputs. Missing quotes reduce coverage; missing quotes for a proven
  upper/lower member fail atomically;
- the breadth result retains universe, quote, upper-pool and lower-pool
  evidence, uses the oldest quote source instant, and uses the latest parsed
  input observation as its derived observation;
- TDX concept hits project only `block_gn.dat`, retain the file SHA-256
  version, reject unsupported Beijing identity before I/O, and use an
  unambiguous epoch-fraction observation timestamp;
- target-price observations require matching source code and non-empty
  `stockName`, retain `indvAimPriceL` as the lower bound and
  `indvAimPriceT` as the upper bound, and reject partial or contradictory
  fields;
- target-price `mean` is the unweighted arithmetic mean of each report range
  midpoint `(L + T) / 2`. It is not a provider-published consensus value;
- exact first-page `hits=0,size=0,TotalPage=0,data=[]` returns typed
  `VerifiedEmpty` with request identity and batch evidence. Partial-zero shapes
  and a later-page transition from non-empty to zero fail as pagination
  contradictions;
- `TargetPriceRouter` deterministic tests require exactly one complete
  aggregate for the requested instrument/range, matching registered Provider,
  provenance batch ID, aggregate/input evidence and valid source/observation
  ordering before selection.

## THS consensus live admission

The Router-backed live operation completed at
`2026-07-27T13:00:48+08:00`:

```bash
MAGIC_THS_CONSENSUS_CODES=600519.SH \
cargo run -p magic-market-router --example consensus_live --locked --offline
```

The current source title was
`贵州茅台(600519) 盈利预测_F10_同花顺金融服务网`. The parser extracted
and cross-checked the structured code/name pair rather than inferring the name
from a code table. Result:

```text
selected_provider=Tonghuashun
records=1
attempt_provider=Tonghuashun status=Selected
stock=600519.Shanghai name=贵州茅台 estimates=3 contributor_count=None source_at=Some("2026-07-27") batch_id=ths:1785128433.361237000:consensus
consensus_router_status=selected
```

## Eastmoney target-price Provider live admission

The formal Eastmoney Provider operation completed at
`2026-07-27T13:01:13+08:00`:

```bash
MAGIC_EASTMONEY_LIVE_OPERATION=target-price \
MAGIC_EASTMONEY_TARGET_CODE=600519 \
MAGIC_EASTMONEY_TARGET_FROM=2026-01-01 \
MAGIC_EASTMONEY_TARGET_THROUGH=2026-07-27 \
cargo run -p magic-eastmoney-rs --example live_probe --locked --offline
```

Result:

```text
status=admitted
stock=600519.SH name=贵州茅台 samples=6 contributors=4
observation_period=2026-01-09..2026-07-20
low=1430 mean=1624.2416666666668 high=1865
mean_semantics=arithmetic_mean_of_report_range_midpoints
source_at=Some("2026-07-20 00:00:00.000")
observed_at=unix-ms:1785128461215
batch_id=eastmoney-web:target-price:unix-ms:1785128461215
input_evidence=6
live_probe_status=admitted
```

This command exercises `EastmoneyClient` directly. It does not claim a
Router-backed live selection; the provider-neutral `TargetPriceRouter`
admission boundary is proved by the deterministic routing suite above.

One live interval row proved distinct lower/upper fields:

```text
report=AP202603191820648186 institution=国信证券
source_indvAimPriceT=1865 source_indvAimPriceL=1686
normalized_low=1686 normalized_high=1865
```

## Full-market ranking remains unadmitted

The first live attempt proved that `push2.eastmoney.com` caps `diff` at 100
rows even when `pz=500`. The production page size was corrected to 100 and a
regression test locks that source boundary.

The subsequent complete `VolumeRatio` attempt advanced through six full pages,
then page 7 failed with a TLS unexpected-EOF transport error. No complete
5,541-security universe was produced:

```text
family=market_rankings.VolumeRatio
status=failed
page=7
error=peer closed connection without sending TLS close_notify
```

After adding the bounded transport-only retry, another live run exhausted all
three paced attempts on page 1 with the same typed TLS unexpected-EOF. The
separately allowlisted Eastmoney HTTPS host
`push2delay.eastmoney.com` was then verified against the same response schema
and added as a whole-operation fallback. A transport failure discards every
partial primary page and restarts at alternate page one; deterministic tests
prove that pages from the two hosts are never joined.

The alternate host completed transport for the 5,541-row intraday universe,
but strict normalization found source movement across the 56 paced pages:

```text
family=market_rankings.VolumeRatio
status=failed
error=market ranking contains duplicate instrument 688651
```

This is an expected atomic rejection of a moving intraday ranking, not a record
to drop or deduplicate. The endpoint also caps `diff` at 100 for requested page
sizes 500 and 1,000, so a larger atomic page is unavailable. A stable
post-close full-universe pass is still required before either metric is
admitted.

The normalized ranking contract now also rejects any non-zero row source-time
skew and uses the common (therefore conservative) time for batch provenance.
This closes the earlier loophole where a numerically representable but
unbounded skew could still produce a strict batch.

The final 2026-07-27 post-close reruns completed all 56 pages through the
alternate host. They found a second independent non-admission boundary: the
source includes securities whose requested metric is explicitly absent.

```text
=== market_rankings.VolumeRatio ===
admitted=false pending_current_live_review
status=failed
error=Eastmoney protocol error: market ranking f10 is absent

=== market_rankings.MainNetInflow ===
admitted=false pending_current_live_review
status=failed
error=Eastmoney protocol error: market ranking f62 is absent
```

Direct page-56 inspection returned `total=5541`, 41 rows, and 20 rows with
`f10="-"`. The same page carried 19 distinct `f124` values from
`2026-07-27T08:00:00+08:00` through `2026-07-27T16:11:58+08:00`.
The official page scripts expose `f297` as a per-security latest trading date,
but they do not expose a full-market common snapshot time. The dated
`stock/fflow/daykline/get` route is one-security-at-a-time and therefore cannot
be relabelled as a provider-native atomic market ranking.

Therefore both independent ranking capabilities and
`SignalCapabilities.market_rankings` remain `false`. In particular,
the project does not claim full Shanghai/Shenzhen/Beijing coverage, 100%
coverage, or a current-session ranking from a partial first page.

## Final focused gates

After the live attempts, formatting, all focused suites, and all-target Clippy
passed:

```bash
cargo fmt --all -- --check
cargo clippy -p magic-market-core -p magic-market-analysis \
  -p magic-eastmoney-rs -p magic-tdx-rs -p magic-ths-rs \
  -p magic-market-router --all-targets --locked --offline -- -D warnings
```

Focused results included Core ranking `6`, Core research `4`, Core target price
`4`, breadth `9`, Eastmoney ranking unit `11`, Eastmoney target price `4`,
Eastmoney reports `9`, TDX concept integration `2`, TDX block projection `9`,
THS consensus `6`, Router consensus example `2`, and target-price routing `7`,
all passed.

## Strict post-close diagnostic

The dedicated operation was first exercised at 13:01 Asia/Shanghai:

```bash
MAGIC_EASTMONEY_LIVE_OPERATION=post-close-ranking \
MAGIC_EASTMONEY_POST_CLOSE_DATE=2026-07-27 \
MAGIC_EASTMONEY_POST_CLOSE_LIMIT=20 \
cargo run -p magic-eastmoney-rs --example live_probe --locked --offline
```

It failed before network I/O as required:

```text
status=failed
error=invalid request: post-close ranking cannot be captured before 15:35:00 Asia/Shanghai
```

This is capture-window evidence, not a same-day post-15:35 data admission.

The same operation was rerun after 15:35 through the canonical primary and
alternate HTTPS endpoints. The primary endpoint returned an empty response;
the alternate endpoint returned rows whose per-security `f124` values differed.
After the production/diagnostic split, the final command completed with no
production-success marker:

```text
=== capital.post_close_ranking ===
admitted=false
status=failed
error=Eastmoney protocol error: post-close source timestamps differ inside one ranking

=== diagnostic_summary ===
diagnostic_probe_status=unadmitted
```

Therefore `CapitalCapabilities.post_close_flow=false`; formal
`PostCloseFlows::post_close_flows` returns typed `Unsupported`, while
`EastmoneyClient::diagnose_post_close_flows` retains the strict parser and
bounded transport diagnostics. No per-security fetch time or local observation
time is substituted for one provider-published batch timestamp.
