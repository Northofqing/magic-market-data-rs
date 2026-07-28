#![cfg(unix)]

use magic_emquant_rs::EmQuantClient;
use magic_market_core::{
    Adjustment, AssetClass, Auctions, BarInterval, BarsRequest, DataStatus, Exchange,
    HistoricalBars, InstrumentId, Money, MoneyFlows, OrderBooks, Price, Quantity, RealtimeQuotes,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn fixture_bridge(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    let metadata = fs::metadata(&path).expect("checked-in fake bridge metadata");
    assert!(metadata.is_file(), "fake bridge must be a regular file");
    assert_ne!(
        metadata.permissions().mode() & 0o111,
        0,
        "checked-in fake bridge must be executable"
    );
    path
}

fn fake_bridge() -> PathBuf {
    fixture_bridge("default_bridge.sh")
}

fn instruments() -> [InstrumentId; 2] {
    [
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap(),
    ]
}

#[test]
fn terminates_a_hung_bridge_at_the_configured_timeout() {
    let client = EmQuantClient::new(fixture_bridge("hung_bridge.sh"))
        .unwrap()
        .with_timeout(Duration::from_millis(25))
        .unwrap();

    let error = client.realtime_quotes(&instruments()).unwrap_err();
    assert!(error.to_string().contains("timed out after 25 ms"));
}

#[test]
fn executes_bridge_and_normalizes_quotes_in_request_order() {
    let client = EmQuantClient::new(fake_bridge()).unwrap();
    let batch = client.realtime_quotes(&instruments()).unwrap();

    assert!(client.capabilities().quotes);
    assert!(client.capabilities().order_book);
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].instrument().code(), "600519");
    assert_eq!(batch.records()[0].price(), Price::new(1300.0).unwrap());
    assert_eq!(batch.records()[0].name(), Some("贵州茅台"));
    assert_eq!(
        batch.records()[0].previous_close(),
        Some(Price::new(1290.0).unwrap())
    );
    assert_eq!(batch.records()[0].open(), Some(Price::new(1295.0).unwrap()));
    assert_eq!(batch.records()[0].high(), Some(Price::new(1305.0).unwrap()));
    assert_eq!(batch.records()[0].low(), Some(Price::new(1288.0).unwrap()));
    assert_eq!(batch.records()[0].status(), DataStatus::Available);
    assert_eq!(batch.records()[0].volume(), Quantity::new(100.0).unwrap());
    assert_eq!(
        batch.records()[0].amount(),
        Some(Money::new(130_000.0).unwrap())
    );
    assert_eq!(batch.records()[0].source_at(), Some("2026-07-22 10:00:00"));
    assert_eq!(batch.records()[1].instrument().code(), "000001");
    assert_eq!(batch.provenance().source(), "eastmoney-emquant");
    assert!(batch.records()[0].batch_id().ends_with(":quote"));
    assert_eq!(
        batch.records()[0].batch_id(),
        batch.provenance().batch_id().unwrap()
    );
    assert!(batch.quality().is_complete());
}

#[test]
fn executes_bridge_and_preserves_missing_order_book_levels() {
    let client = EmQuantClient::new(fake_bridge()).unwrap();
    let batch = client.order_books(&instruments()).unwrap();

    assert_eq!(batch.records().len(), 2);
    let first = &batch.records()[0];
    assert_eq!(first.instrument().code(), "600519");
    assert_eq!(first.status(), DataStatus::Unavailable);
    assert_eq!(first.bids()[0].price().map(Price::get), Some(1299.0));
    assert_eq!(first.asks()[1].quantity().map(Quantity::get), Some(13.0));
    assert!(first.bids()[2].price().is_none());
    assert!(first.bids()[2].quantity().is_none());
    assert_eq!(first.total_bid_quantity().map(Quantity::get), Some(22.0));
    assert_eq!(first.total_ask_quantity().map(Quantity::get), Some(24.0));
    assert_eq!(first.source_at(), Some("2026-07-22 10:00:00"));
    assert_eq!(first.provider(), magic_market_core::ProviderId::Eastmoney);
    assert!(first.batch_id().ends_with(":order-book"));
    assert_eq!(first.batch_id(), batch.provenance().batch_id().unwrap());
    assert!(!batch.quality().is_complete());
}

#[test]
fn executes_csd_and_returns_bounded_normalized_bars() {
    let client = EmQuantClient::new(fake_bridge()).unwrap();
    let request = BarsRequest::new(instruments()[0].clone(), BarInterval::Day, 2)
        .unwrap()
        .with_range("2026-07-20", "2026-07-22")
        .unwrap();
    let batch = client.historical_bars(&request).unwrap();

    assert!(client.capabilities().bars);
    assert!(client.capabilities().minute);
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].bar_start(), "2026-07-21");
    assert_eq!(batch.records()[0].open(), Price::new(1300.0).unwrap());
    assert_eq!(batch.records()[1].close(), Price::new(1320.0).unwrap());
    assert_eq!(batch.records()[1].adjustment(), Adjustment::Unadjusted);
    assert_eq!(batch.provenance().source_at(), Some("2026-07-22"));
    assert!(batch.quality().is_complete());
}

#[test]
fn executes_chmc_and_aggregates_five_minute_bars() {
    let client = EmQuantClient::new(fake_bridge()).unwrap();
    let request = BarsRequest::new(instruments()[0].clone(), BarInterval::Minute5, 1)
        .unwrap()
        .with_range("2026-07-22", "2026-07-22")
        .unwrap();
    let batch = client.historical_bars(&request).unwrap();

    assert_eq!(batch.records().len(), 1);
    let bar = &batch.records()[0];
    assert_eq!(bar.interval(), BarInterval::Minute5);
    assert_eq!(bar.bar_start(), "2026-07-22 09:30:00");
    assert_eq!(bar.bar_end(), "2026-07-22 09:34:00");
    assert_eq!(bar.open(), Price::new(1300.0).unwrap());
    assert_eq!(bar.high(), Price::new(1306.0).unwrap());
    assert_eq!(bar.low(), Price::new(1299.0).unwrap());
    assert_eq!(bar.close(), Price::new(1305.0).unwrap());
    assert_eq!(bar.volume(), Quantity::new(60.0).unwrap());
    assert_eq!(bar.amount(), Some(Money::new(78_190.0).unwrap()));
    assert_eq!(batch.provenance().source_at(), Some("2026-07-22 09:34:00"));
}

#[test]
fn executes_css_and_normalizes_daily_money_flow() {
    let client = EmQuantClient::new(fake_bridge()).unwrap();
    let batch = client.money_flows(&instruments()).unwrap();

    assert!(client.capabilities().money_flow);
    assert_eq!(batch.records().len(), 2);
    let first = &batch.records()[0];
    assert_eq!(first.instrument().code(), "600519");
    assert_eq!(first.main_net(), Some(Money::new(110.0).unwrap()));
    assert_eq!(first.super_large_net(), Some(Money::new(60.0).unwrap()));
    assert_eq!(first.large_net(), Some(Money::new(50.0).unwrap()));
    assert_eq!(first.medium_net(), Some(Money::new(-5.0).unwrap()));
    assert_eq!(first.small_net(), Some(Money::new(-10.0).unwrap()));
    assert_eq!(first.status(), DataStatus::Available);
    assert_eq!(first.source_at(), Some("2026-07-22"));
    assert!(batch.quality().is_complete());
}

#[test]
fn opening_auction_is_explicitly_unsupported() {
    let client = EmQuantClient::new(fake_bridge()).unwrap();
    let error = client.auction_snapshots(&instruments()).unwrap_err();
    assert!(error.to_string().contains("opening-auction"));
    assert!(!client.capabilities().auction);
}

#[test]
fn rejects_reversed_csd_dates_instead_of_sorting_them() {
    let client = EmQuantClient::new(fixture_bridge("reversed_bars_bridge.sh")).unwrap();
    let request = BarsRequest::new(instruments()[0].clone(), BarInterval::Day, 2)
        .unwrap()
        .with_range("2026-07-20", "2026-07-22")
        .unwrap();

    let error = client.historical_bars(&request).unwrap_err();
    assert!(error.to_string().contains("duplicated or out of order"));
}

#[test]
fn executes_one_checked_in_immutable_fake_bridge_in_parallel() {
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let record_counts = std::thread::scope(|scope| {
        let handles = (0..8)
            .map(|_| {
                let barrier = std::sync::Arc::clone(&barrier);
                scope.spawn(move || {
                    let client = EmQuantClient::new(fake_bridge()).expect("construct fake client");
                    barrier.wait();
                    client
                        .realtime_quotes(&instruments())
                        .expect("execute immutable fake bridge")
                        .records()
                        .len()
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("parallel fake bridge thread"))
            .collect::<Vec<_>>()
    });
    assert_eq!(record_counts, vec![2; 8]);
}
