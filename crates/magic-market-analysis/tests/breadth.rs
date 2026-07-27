use magic_market_analysis::{
    BreadthAnalysis, BreadthLimitPool, BreadthUniverse, MarketBreadthAnalysis,
};
use magic_market_core::Provenance;
use magic_market_core::{
    AssetClass, DataBatch, DataStatus, Exchange, InstrumentId, IsoDate, LimitPoolEntry,
    LimitPoolKind, MarketBreadth, MarketBreadthRequest, MarketSession, NonEmptyText, Price,
    PriceLimitRule, ProviderId, Quantity, Quote, Ratio, RatioUnit, SecurityMetadata,
    SourceEvidence,
};

const DATE: &str = "2026-07-27";
const OBSERVED: &str = "2026-07-27T10:00:02+08:00";
const LOWER_POOL_OBSERVED: &str = "2026-07-27T10:00:03+08:00";
const UPPER_POOL_OBSERVED: &str = "2026-07-27T10:00:04+08:00";
const QUOTE_BATCH: &str = "input-quotes";

fn instrument(code: &str) -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap()
}

fn quote(
    code: &str,
    price: f64,
    previous: Option<f64>,
    source_at: &str,
    observed_at: &str,
    provider: ProviderId,
    batch: &str,
) -> Quote {
    Quote::from_parts(
        instrument(code),
        Some(format!("名称{code}")),
        Price::new(price).unwrap(),
        previous.map(|value| Price::new(value).unwrap()),
        None,
        None,
        None,
        None,
        Quantity::new(100.0).unwrap(),
        None,
        DataStatus::Unavailable,
        Some(source_at.into()),
        observed_at,
        provider,
        batch,
    )
    .unwrap()
}

fn quote_batch(
    records: Vec<Quote>,
    conservative_source_at: &str,
    observed_at: &str,
    batch_id: &str,
) -> DataBatch<Quote> {
    DataBatch::strict(
        records,
        Provenance::new("tencent-web", observed_at)
            .unwrap()
            .with_source_at(conservative_source_at)
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    )
}

fn limit_entry(
    kind: LimitPoolKind,
    code: &str,
    date: &str,
    observed_at: &str,
    provider: ProviderId,
    batch_id: &str,
) -> LimitPoolEntry {
    LimitPoolEntry {
        kind,
        instrument: instrument(code),
        trading_date: IsoDate::new(date).unwrap(),
        price: Price::new(10.0).unwrap(),
        change: Ratio::new(
            if kind == LimitPoolKind::Upper {
                10.0
            } else {
                -10.0
            },
            RatioUnit::Percent,
        )
        .unwrap(),
        volume: None,
        turnover: None,
        sealed_amount: None,
        first_seal_at: None,
        last_seal_at: None,
        break_count: None,
        streak: None,
        industry: None,
        board_name: None,
        seal_state: None,
        reseal_count: None,
        reason: None,
        evidence: SourceEvidence::new(provider, observed_at, batch_id)
            .unwrap()
            .with_source_at(date)
            .unwrap(),
    }
}

fn limit_pool(
    kind: LimitPoolKind,
    codes: &[&str],
    date: &str,
    observed_at: &str,
    batch_id: &str,
) -> BreadthLimitPool {
    let records = codes
        .iter()
        .map(|code| {
            limit_entry(
                kind,
                code,
                date,
                observed_at,
                ProviderId::Eastmoney,
                batch_id,
            )
        })
        .collect();
    let batch = DataBatch::strict(
        records,
        Provenance::new("eastmoney-web", observed_at)
            .unwrap()
            .with_source_at(date)
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap(),
    );
    BreadthLimitPool::new(
        kind,
        IsoDate::new(date).unwrap(),
        ProviderId::Eastmoney,
        batch,
    )
    .unwrap()
}

fn analysis(
    quotes: DataBatch<Quote>,
    upper: &[&str],
    lower: &[&str],
) -> Result<MarketBreadthAnalysis, magic_market_analysis::AnalysisError> {
    let codes = quotes
        .records()
        .iter()
        .map(|quote| quote.instrument().code().to_owned())
        .collect::<Vec<_>>();
    analysis_with_universe(&codes, quotes, upper, lower)
}

fn universe(codes: &[String]) -> BreadthUniverse {
    let batch_id = "universe-v1";
    let records = codes
        .iter()
        .map(|code| {
            SecurityMetadata::new(
                instrument(code),
                Some(format!("名称{code}")),
                None,
                None,
                None,
                PriceLimitRule::new(None, None).unwrap(),
                DataStatus::Unavailable,
                Some(DATE.into()),
                OBSERVED,
                ProviderId::Sina,
                batch_id,
            )
            .unwrap()
        })
        .collect();
    BreadthUniverse::new(
        NonEmptyText::new("A-share-equities").unwrap(),
        IsoDate::new(DATE).unwrap(),
        NonEmptyText::new("security-master-v1").unwrap(),
        ProviderId::Sina,
        DataBatch::strict(
            records,
            Provenance::new("sina-web", OBSERVED)
                .unwrap()
                .with_source_at(DATE)
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        ),
    )
    .unwrap()
}

fn analysis_with_universe(
    universe_codes: &[String],
    quotes: DataBatch<Quote>,
    upper: &[&str],
    lower: &[&str],
) -> Result<MarketBreadthAnalysis, magic_market_analysis::AnalysisError> {
    MarketBreadthAnalysis::new(
        universe(universe_codes),
        quotes,
        limit_pool(
            LimitPoolKind::Upper,
            upper,
            DATE,
            UPPER_POOL_OBSERVED,
            "upper-pool",
        ),
        limit_pool(
            LimitPoolKind::Lower,
            lower,
            DATE,
            LOWER_POOL_OBSERVED,
            "lower-pool",
        ),
    )
}

fn request(
    source_date: &str,
    session: MarketSession,
    minimum_coverage: f64,
    maximum_skew: u64,
) -> MarketBreadthRequest {
    MarketBreadthRequest::new(
        NonEmptyText::new("A-share-equities").unwrap(),
        IsoDate::new(source_date).unwrap(),
        session,
        Ratio::decimal(minimum_coverage).unwrap(),
        maximum_skew,
    )
    .unwrap()
}

#[test]
fn computes_source_pure_partition_coverage_skew_and_all_input_evidence() {
    let oldest = "2026-07-27T10:00:00.123+08:00";
    let newest = "2026-07-27T10:00:00.923+08:00";
    let quotes = quote_batch(
        vec![
            quote(
                "600001",
                11.0,
                Some(10.0),
                oldest,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
            quote(
                "600002",
                9.0,
                Some(10.0),
                newest,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
            quote(
                "600003",
                10.0,
                Some(10.0),
                oldest,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
            quote(
                "600004",
                12.0,
                None,
                newest,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
        ],
        oldest,
        OBSERVED,
        QUOTE_BATCH,
    );
    let result = analysis(quotes, &["600001"], &["600002"])
        .unwrap()
        .market_breadth(&request(DATE, MarketSession::Continuous, 0.75, 800))
        .unwrap();
    let snapshot = &result.records()[0];
    assert_eq!(snapshot.total(), 4);
    assert_eq!(snapshot.valid(), 3);
    assert_eq!((snapshot.up(), snapshot.down(), snapshot.flat()), (1, 1, 1));
    assert_eq!((snapshot.limit_up(), snapshot.limit_down()), (1, 1));
    assert_eq!(snapshot.coverage().get(), 0.75);
    assert_eq!(snapshot.max_source_skew_millis(), 800);
    assert_eq!(snapshot.input_evidence().len(), 4);
    assert_eq!(snapshot.input_evidence()[1].source_at(), Some(oldest));
    assert_eq!(
        snapshot
            .input_evidence()
            .iter()
            .map(|evidence| evidence.batch_id())
            .collect::<Vec<_>>(),
        vec!["universe-v1", QUOTE_BATCH, "upper-pool", "lower-pool"]
    );
    assert_eq!(result.provenance().source_at(), Some(oldest));
    assert_eq!(result.provenance().fetched_at(), UPPER_POOL_OBSERVED);
}

#[test]
fn missing_previous_close_is_the_only_exclusion_and_keeps_universe_denominator() {
    let source_at = "2026-07-27T10:00:00+08:00";
    let quotes = quote_batch(
        vec![
            quote(
                "600001",
                11.0,
                Some(10.0),
                source_at,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
            quote(
                "600002",
                10.0,
                None,
                source_at,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
        ],
        source_at,
        OBSERVED,
        QUOTE_BATCH,
    );
    let result = analysis(quotes, &[], &[])
        .unwrap()
        .market_breadth(&request(DATE, MarketSession::Continuous, 0.5, 0))
        .unwrap();
    assert_eq!(
        (result.records()[0].total(), result.records()[0].valid()),
        (2, 1)
    );
}

#[test]
fn missing_quotes_are_invalid_against_the_proved_universe_denominator() {
    let source_at = "2026-07-27T10:00:00+08:00";
    let quotes = quote_batch(
        vec![quote(
            "600001",
            11.0,
            Some(10.0),
            source_at,
            OBSERVED,
            ProviderId::Tencent,
            QUOTE_BATCH,
        )],
        source_at,
        OBSERVED,
        QUOTE_BATCH,
    );
    let result = analysis_with_universe(
        &["600001".to_owned(), "600002".to_owned()],
        quotes,
        &[],
        &[],
    )
    .unwrap()
    .market_breadth(&request(DATE, MarketSession::Continuous, 0.5, 0))
    .unwrap();
    assert_eq!(result.records()[0].total(), 2);
    assert_eq!(result.records()[0].valid(), 1);
    assert_eq!(result.records()[0].coverage().get(), 0.5);
}

#[test]
fn rejects_duplicate_quotes_unknown_or_conflicting_limit_members_and_wrong_direction() {
    let source_at = "2026-07-27T10:00:00+08:00";
    let duplicate = quote_batch(
        vec![
            quote(
                "600001",
                11.0,
                Some(10.0),
                source_at,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
            quote(
                "600001",
                12.0,
                Some(10.0),
                source_at,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
        ],
        source_at,
        OBSERVED,
        QUOTE_BATCH,
    );
    assert!(analysis_with_universe(&["600001".to_owned()], duplicate, &[], &[]).is_err());

    let one_quote = || {
        quote_batch(
            vec![quote(
                "600001",
                9.0,
                Some(10.0),
                source_at,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            )],
            source_at,
            OBSERVED,
            QUOTE_BATCH,
        )
    };
    assert!(analysis(one_quote(), &["600002"], &[]).is_err());
    assert!(analysis(one_quote(), &["600001"], &["600001"]).is_err());
    assert!(analysis(one_quote(), &["600001"], &[])
        .unwrap()
        .market_breadth(&request(DATE, MarketSession::Continuous, 1.0, 0))
        .is_err());
}

#[test]
fn rejects_insufficient_coverage_excessive_skew_wrong_date_or_session() {
    let oldest = "2026-07-27T10:00:00+08:00";
    let newest = "2026-07-27T10:00:02+08:00";
    let quotes = quote_batch(
        vec![
            quote(
                "600001",
                11.0,
                Some(10.0),
                oldest,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
            quote(
                "600002",
                11.0,
                None,
                newest,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
        ],
        oldest,
        OBSERVED,
        QUOTE_BATCH,
    );
    let analysis = analysis(quotes, &[], &[]).unwrap();
    assert!(analysis
        .market_breadth(&request(DATE, MarketSession::Continuous, 0.75, 2_000))
        .is_err());
    assert!(analysis
        .market_breadth(&request(DATE, MarketSession::Continuous, 0.5, 1_999))
        .is_err());
    assert!(analysis
        .market_breadth(&request(
            "2026-07-26",
            MarketSession::Continuous,
            0.5,
            2_000
        ))
        .is_err());
    assert!(analysis
        .market_breadth(&request(DATE, MarketSession::PreOpen, 0.5, 2_000))
        .is_err());
}

#[test]
fn rejects_non_atomic_quote_evidence_future_sources_and_ambiguous_instants() {
    let source_at = "2026-07-27T10:00:00+08:00";
    for records in [
        vec![
            quote(
                "600001",
                11.0,
                Some(10.0),
                source_at,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
            quote(
                "600002",
                9.0,
                Some(10.0),
                source_at,
                "2026-07-27T10:00:03+08:00",
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
        ],
        vec![quote(
            "600001",
            11.0,
            Some(10.0),
            source_at,
            OBSERVED,
            ProviderId::Tencent,
            "wrong-batch",
        )],
        vec![
            quote(
                "600001",
                11.0,
                Some(10.0),
                source_at,
                OBSERVED,
                ProviderId::Tencent,
                QUOTE_BATCH,
            ),
            quote(
                "600002",
                9.0,
                Some(10.0),
                source_at,
                OBSERVED,
                ProviderId::Eastmoney,
                QUOTE_BATCH,
            ),
        ],
    ] {
        assert!(analysis(
            quote_batch(records, source_at, OBSERVED, QUOTE_BATCH),
            &[],
            &[]
        )
        .unwrap()
        .market_breadth(&request(DATE, MarketSession::Continuous, 1.0, 0))
        .is_err());
    }

    for bad_source in [
        "2026-07-27T10:00:03+08:00",
        "2026-07-27T10:00:00",
        "2026-07-27T10:00:00+09:00",
    ] {
        assert!(analysis(
            quote_batch(
                vec![quote(
                    "600001",
                    11.0,
                    Some(10.0),
                    bad_source,
                    OBSERVED,
                    ProviderId::Tencent,
                    QUOTE_BATCH,
                )],
                bad_source,
                OBSERVED,
                QUOTE_BATCH,
            ),
            &[],
            &[],
        )
        .unwrap()
        .market_breadth(&request(DATE, MarketSession::Continuous, 1.0, 0))
        .is_err());
    }
}

#[test]
fn limit_pool_input_is_request_typed_atomic_and_preserves_verified_empty_evidence() {
    let empty = limit_pool(LimitPoolKind::Upper, &[], DATE, OBSERVED, "empty-upper");
    assert!(empty.entries().records().is_empty());
    assert_eq!(empty.kind(), LimitPoolKind::Upper);
    assert_eq!(empty.evidence().batch_id(), "empty-upper");

    let wrong_record = limit_entry(
        LimitPoolKind::Upper,
        "600001",
        DATE,
        OBSERVED,
        ProviderId::Tencent,
        "upper-pool",
    );
    let wrong_batch = DataBatch::strict(
        vec![wrong_record],
        Provenance::new("eastmoney-web", OBSERVED)
            .unwrap()
            .with_source_at(DATE)
            .unwrap()
            .with_batch_id("upper-pool")
            .unwrap(),
    );
    assert!(BreadthLimitPool::new(
        LimitPoolKind::Upper,
        IsoDate::new(DATE).unwrap(),
        ProviderId::Eastmoney,
        wrong_batch,
    )
    .is_err());
}

#[test]
fn every_limit_pool_member_requires_a_valid_directional_quote() {
    let source_at = "2026-07-27T10:00:00+08:00";
    let only_first = quote_batch(
        vec![quote(
            "600001",
            11.0,
            Some(10.0),
            source_at,
            OBSERVED,
            ProviderId::Tencent,
            QUOTE_BATCH,
        )],
        source_at,
        OBSERVED,
        QUOTE_BATCH,
    );
    assert!(analysis_with_universe(
        &["600001".to_owned(), "600002".to_owned()],
        only_first,
        &["600002"],
        &[],
    )
    .unwrap()
    .market_breadth(&request(DATE, MarketSession::Continuous, 0.5, 0))
    .is_err());

    let missing_previous = quote_batch(
        vec![quote(
            "600001",
            11.0,
            None,
            source_at,
            OBSERVED,
            ProviderId::Tencent,
            QUOTE_BATCH,
        )],
        source_at,
        OBSERVED,
        QUOTE_BATCH,
    );
    assert!(analysis(missing_previous, &["600001"], &[])
        .unwrap()
        .market_breadth(&request(DATE, MarketSession::Continuous, 0.0, 0))
        .is_err());
}

fn assert_alias<T: BreadthAnalysis>() {}

#[test]
fn analysis_implements_the_public_breadth_alias() {
    assert_alias::<MarketBreadthAnalysis>();
}
