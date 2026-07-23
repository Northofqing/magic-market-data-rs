use magic_market_core::{
    AuctionSnapshot, Bar, MinutePoint, MoneyFlow, OrderBook, ProviderId, Quote, SecurityMetadata,
    SourcedRecord, Trade,
};

fn assert_sourced<T: SourcedRecord>() {}

#[test]
fn every_normalized_record_exposes_common_evidence() {
    assert_sourced::<Quote>();
    assert_sourced::<Bar>();
    assert_sourced::<MinutePoint>();
    assert_sourced::<Trade>();
    assert_sourced::<MoneyFlow>();
    assert_sourced::<OrderBook>();
    assert_sourced::<AuctionSnapshot>();
    assert_sourced::<SecurityMetadata>();

    let provider: fn(&Quote) -> ProviderId = SourcedRecord::provider_id;
    let batch: fn(&Quote) -> &str = SourcedRecord::evidence_batch_id;
    let _ = (provider, batch);
}
