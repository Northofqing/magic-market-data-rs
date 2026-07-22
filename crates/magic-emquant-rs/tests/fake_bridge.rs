#![cfg(unix)]

use magic_emquant_rs::EmQuantClient;
use magic_market_core::{
    Adjustment, AssetClass, BarInterval, BarsRequest, DataStatus, Exchange, HistoricalBars,
    InstrumentId, Money, OrderBooks, Price, Quantity, RealtimeQuotes,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn fake_bridge() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("magic-emquant-fake-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&directory).expect("create fake bridge directory");
    let bridge = directory.join("snapshot");
    fs::write(
        &bridge,
        r##"#!/bin/sh
set -eu
if test "$1" = "--history"; then
  test "$2" = "csd"
  test "$3" = "600519.SH"
  test "$4" = "OPEN,HIGH,LOW,CLOSE,VOLUME,AMOUNT"
  test "$5" = "2026-07-20"
  test "$6" = "2026-07-22"
  test "$7" = "Period=1,AdjustFlag=1,Order=1"
  printf '%s\n' '{"records":[{"date":"2026-07-20","code":"600519.SH","values":{"OPEN":1290,"HIGH":1310,"LOW":1288,"CLOSE":1300,"VOLUME":100,"AMOUNT":130000}},{"date":"2026-07-21","code":"600519.SH","values":{"OPEN":1300,"HIGH":1320,"LOW":1298,"CLOSE":1310,"VOLUME":110,"AMOUNT":144100}},{"date":"2026-07-22","code":"600519.SH","values":{"OPEN":1310,"HIGH":1330,"LOW":1308,"CLOSE":1320,"VOLUME":120,"AMOUNT":158400}}]}'
  exit 0
fi
test "$1" = "600519.SH,000001.SZ"
if test "$2" = "TIME,NOW,VOLUME,AMOUNT"; then
  printf '%s\n' '{"records":[{"date":"2026-07-22","code":"000001.SZ","values":{"TIME":"10:00:01","NOW":12.5,"VOLUME":200,"AMOUNT":2500}},{"date":"2026-07-22","code":"600519.SH","values":{"TIME":"10:00:00","NOW":1300,"VOLUME":100,"AMOUNT":130000}}]}'
else
  printf '%s\n' '{"records":[{"date":"2026-07-22","code":"000001.SZ","values":{"TIME":"10:00:01","BUYPRICE1":12.49,"BUYVOLUME1":20,"SELLPRICE1":12.51,"SELLVOLUME1":30}},{"date":"2026-07-22","code":"600519.SH","values":{"TIME":"10:00:00","BUYPRICE1":1299,"BUYVOLUME1":10,"SELLPRICE1":1301,"SELLVOLUME1":11,"BUYPRICE2":1298,"BUYVOLUME2":12,"SELLPRICE2":1302,"SELLVOLUME2":13}}]}'
fi
"##,
    )
    .expect("write fake bridge");
    let mut permissions = fs::metadata(&bridge)
        .expect("bridge metadata")
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&bridge, permissions).expect("make bridge executable");
    bridge
}

fn instruments() -> [InstrumentId; 2] {
    [
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap(),
    ]
}

#[test]
fn terminates_a_hung_bridge_at_the_configured_timeout() {
    let bridge = fake_bridge();
    fs::write(&bridge, "#!/bin/sh\nexec sleep 5\n").unwrap();
    let client = EmQuantClient::new(bridge)
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
    assert_eq!(batch.records()[0].instrument.code(), "600519");
    assert_eq!(batch.records()[0].price, Price::new(1300.0).unwrap());
    assert_eq!(batch.records()[0].volume, Quantity::new(100.0).unwrap());
    assert_eq!(
        batch.records()[0].amount,
        Some(Money::new(130_000.0).unwrap())
    );
    assert_eq!(
        batch.records()[0].source_at.as_deref(),
        Some("2026-07-22 10:00:00")
    );
    assert_eq!(batch.records()[1].instrument.code(), "000001");
    assert_eq!(batch.provenance().source, "eastmoney-emquant");
    assert!(batch.quality().complete);
}

#[test]
fn executes_bridge_and_preserves_missing_order_book_levels() {
    let client = EmQuantClient::new(fake_bridge()).unwrap();
    let batch = client.order_books(&instruments()).unwrap();

    assert_eq!(batch.records().len(), 2);
    let first = &batch.records()[0];
    assert_eq!(first.instrument.code(), "600519");
    assert_eq!(first.status, DataStatus::Available);
    assert_eq!(first.bids[0].price.map(Price::get), Some(1299.0));
    assert_eq!(first.asks[1].quantity.map(Quantity::get), Some(13.0));
    assert!(first.bids[2].price.is_none());
    assert!(first.bids[2].quantity.is_none());
    assert!(batch.quality().complete);
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
    assert!(!client.capabilities().minute);
    assert_eq!(batch.records().len(), 2);
    assert_eq!(batch.records()[0].bar_start, "2026-07-21");
    assert_eq!(batch.records()[0].open, Price::new(1300.0).unwrap());
    assert_eq!(batch.records()[1].close, Price::new(1320.0).unwrap());
    assert_eq!(batch.records()[1].adjustment, Adjustment::Unadjusted);
    assert_eq!(batch.provenance().source_at.as_deref(), Some("2026-07-22"));
    assert!(batch.quality().complete);
}

#[test]
fn rejects_reversed_csd_dates_instead_of_sorting_them() {
    let bridge = fake_bridge();
    fs::write(
        &bridge,
        r##"#!/bin/sh
set -eu
printf '%s\n' '{"records":[{"date":"2026-07-22","code":"600519.SH","values":{"OPEN":1310,"HIGH":1330,"LOW":1308,"CLOSE":1320,"VOLUME":120,"AMOUNT":158400}},{"date":"2026-07-21","code":"600519.SH","values":{"OPEN":1300,"HIGH":1320,"LOW":1298,"CLOSE":1310,"VOLUME":110,"AMOUNT":144100}}]}'
"##,
    )
    .unwrap();
    let client = EmQuantClient::new(bridge).unwrap();
    let request = BarsRequest::new(instruments()[0].clone(), BarInterval::Day, 2)
        .unwrap()
        .with_range("2026-07-20", "2026-07-22")
        .unwrap();

    let error = client.historical_bars(&request).unwrap_err();
    assert!(error.to_string().contains("duplicated or out of order"));
}
