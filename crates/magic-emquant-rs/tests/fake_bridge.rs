#![cfg(unix)]

use magic_emquant_rs::EmQuantClient;
use magic_market_core::{
    Adjustment, AssetClass, Auctions, BarInterval, BarsRequest, DataStatus, Exchange,
    HistoricalBars, InstrumentId, Money, MoneyFlows, OrderBooks, Price, Quantity, RealtimeQuotes,
};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static NEXT_BRIDGE_ID: AtomicU64 = AtomicU64::new(0);

const DEFAULT_BRIDGE_SCRIPT: &str = r##"#!/bin/sh
set -eu
if test "$1" = "--section"; then
  test "$2" = "css"
  test "$3" = "600519.SH,000001.SZ"
  test "$4" = "SUPERINFLOW,SUPEROUTFLOW,BIGINFLOW,BIGOUTFLOW,MIDINFLOW,MIDOUTFLOW,SMALLINFLOW,SMALLOUTFLOW"
  test "$5" = ""
  printf '%s\n' '{"records":[{"date":"2026-07-22","code":"000001.SZ","values":{"SUPERINFLOW":50,"SUPEROUTFLOW":40,"BIGINFLOW":30,"BIGOUTFLOW":20,"MIDINFLOW":20,"MIDOUTFLOW":10,"SMALLINFLOW":10,"SMALLOUTFLOW":20}},{"date":"2026-07-22","code":"600519.SH","values":{"SUPERINFLOW":100,"SUPEROUTFLOW":40,"BIGINFLOW":80,"BIGOUTFLOW":30,"MIDINFLOW":20,"MIDOUTFLOW":25,"SMALLINFLOW":10,"SMALLOUTFLOW":20}}]}'
  exit 0
fi
if test "$1" = "--history"; then
  if test "$2" = "chmc"; then
    test "$3" = "600519.SH"
    test "$4" = "DATE,TIME,OPEN,HIGH,LOW,CLOSE,VOLUME,AMOUNT"
    test "$5" = "2026-07-22"
    test "$6" = "2026-07-22"
    test "$7" = ""
    printf '%s\n' '{"records":[{"date":"2026-07-22","code":"600519.SH","values":{"DATE":"20260722","TIME":"09:30:00","OPEN":1300,"HIGH":1302,"LOW":1299,"CLOSE":1301,"VOLUME":10,"AMOUNT":13010}},{"date":"2026-07-22","code":"600519.SH","values":{"DATE":"20260722","TIME":"09:31:00","OPEN":1301,"HIGH":1303,"LOW":1300,"CLOSE":1302,"VOLUME":11,"AMOUNT":14322}},{"date":"2026-07-22","code":"600519.SH","values":{"DATE":"20260722","TIME":"09:32:00","OPEN":1302,"HIGH":1304,"LOW":1301,"CLOSE":1303,"VOLUME":12,"AMOUNT":15636}},{"date":"2026-07-22","code":"600519.SH","values":{"DATE":"20260722","TIME":"09:33:00","OPEN":1303,"HIGH":1305,"LOW":1302,"CLOSE":1304,"VOLUME":13,"AMOUNT":16952}},{"date":"2026-07-22","code":"600519.SH","values":{"DATE":"20260722","TIME":"09:34:00","OPEN":1304,"HIGH":1306,"LOW":1303,"CLOSE":1305,"VOLUME":14,"AMOUNT":18270}}]}'
    exit 0
  fi
  test "$2" = "csd"
  test "$3" = "600519.SH"
  test "$4" = "OPEN,HIGH,LOW,CLOSE,VOLUME,AMOUNT"
  test "$5" = "2026-07-20"
  test "$6" = "2026-07-22"
  test "$7" = "Period=1,AdjustFlag=1,Order=1"
  printf '%s\n' '{"records":[{"date":"2026/7/20","code":"600519.SH","values":{"OPEN":1290,"HIGH":1310,"LOW":1288,"CLOSE":1300,"VOLUME":100,"AMOUNT":130000}},{"date":"2026/7/21","code":"600519.SH","values":{"OPEN":1300,"HIGH":1320,"LOW":1298,"CLOSE":1310,"VOLUME":110,"AMOUNT":144100}},{"date":"2026/7/22","code":"600519.SH","values":{"OPEN":1310,"HIGH":1330,"LOW":1308,"CLOSE":1320,"VOLUME":120,"AMOUNT":158400}}]}'
  exit 0
fi
test "$1" = "600519.SH,000001.SZ"
if test "$2" = "TIME,NAME,NOW,PRECLOSE,OPEN,HIGH,LOW,PCTCHANGE,VOLUME,AMOUNT"; then
  printf '%s\n' '{"records":[{"date":"2026-07-22","code":"000001.SZ","values":{"TIME":"10:00:01","NAME":"平安银行","NOW":12.5,"PRECLOSE":12.2,"OPEN":12.3,"HIGH":12.6,"LOW":12.1,"PCTCHANGE":2.459,"VOLUME":200,"AMOUNT":2500}},{"date":"2026-07-22","code":"600519.SH","values":{"TIME":"10:00:00","NAME":"贵州茅台","NOW":1300,"PRECLOSE":1290,"OPEN":1295,"HIGH":1305,"LOW":1288,"PCTCHANGE":0.7752,"VOLUME":100,"AMOUNT":130000}}]}'
else
  printf '%s\n' '{"records":[{"date":"2026-07-22","code":"000001.SZ","values":{"TIME":"10:00:01","BUYPRICE1":12.49,"BUYVOLUME1":20,"SELLPRICE1":12.51,"SELLVOLUME1":30}},{"date":"2026-07-22","code":"600519.SH","values":{"TIME":"10:00:00","BUYPRICE1":1299,"BUYVOLUME1":10,"SELLPRICE1":1301,"SELLVOLUME1":11,"BUYPRICE2":1298,"BUYVOLUME2":12,"SELLPRICE2":1302,"SELLVOLUME2":13}}]}'
fi
"##;

const HUNG_BRIDGE_SCRIPT: &str = "#!/bin/sh\nexec sleep 5\n";

const REVERSED_BARS_BRIDGE_SCRIPT: &str = r##"#!/bin/sh
set -eu
printf '%s\n' '{"records":[{"date":"2026-07-22","code":"600519.SH","values":{"OPEN":1310,"HIGH":1330,"LOW":1308,"CLOSE":1320,"VOLUME":120,"AMOUNT":158400}},{"date":"2026-07-21","code":"600519.SH","values":{"OPEN":1300,"HIGH":1320,"LOW":1298,"CLOSE":1310,"VOLUME":110,"AMOUNT":144100}}]}'
"##;

struct FakeBridge {
    directory: PathBuf,
    executable: PathBuf,
}

impl FakeBridge {
    fn new(script: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let id = NEXT_BRIDGE_ID.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "magic-emquant-fake-{}-{nonce}-{id}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create unique fake bridge directory");

        let staging = directory.join("snapshot.staging");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .expect("create fake bridge staging file");
        file.write_all(script.as_bytes())
            .expect("write complete fake bridge");
        file.sync_all().expect("sync complete fake bridge");
        drop(file);

        let mut permissions = fs::metadata(&staging)
            .expect("bridge metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&staging, permissions).expect("make staged bridge executable");

        let executable = directory.join("snapshot");
        fs::rename(&staging, &executable).expect("publish immutable fake bridge");
        Self {
            directory,
            executable,
        }
    }

    fn path(&self) -> &Path {
        &self.executable
    }
}

impl Drop for FakeBridge {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.directory) {
            assert!(
                std::thread::panicking(),
                "remove fake bridge directory {}: {error}",
                self.directory.display()
            );
        }
    }
}

fn fake_bridge() -> FakeBridge {
    FakeBridge::new(DEFAULT_BRIDGE_SCRIPT)
}

fn instruments() -> [InstrumentId; 2] {
    [
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap(),
        InstrumentId::new(Exchange::Shenzhen, "000001", AssetClass::Equity).unwrap(),
    ]
}

#[test]
fn terminates_a_hung_bridge_at_the_configured_timeout() {
    let bridge = FakeBridge::new(HUNG_BRIDGE_SCRIPT);
    let client = EmQuantClient::new(bridge.path())
        .unwrap()
        .with_timeout(Duration::from_millis(25))
        .unwrap();

    let error = client.realtime_quotes(&instruments()).unwrap_err();
    assert!(error.to_string().contains("timed out after 25 ms"));
}

#[test]
fn executes_bridge_and_normalizes_quotes_in_request_order() {
    let bridge = fake_bridge();
    let client = EmQuantClient::new(bridge.path()).unwrap();
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
    let bridge = fake_bridge();
    let client = EmQuantClient::new(bridge.path()).unwrap();
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
    let bridge = fake_bridge();
    let client = EmQuantClient::new(bridge.path()).unwrap();
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
    let bridge = fake_bridge();
    let client = EmQuantClient::new(bridge.path()).unwrap();
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
    let bridge = fake_bridge();
    let client = EmQuantClient::new(bridge.path()).unwrap();
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
    let bridge = fake_bridge();
    let client = EmQuantClient::new(bridge.path()).unwrap();
    let error = client.auction_snapshots(&instruments()).unwrap_err();
    assert!(error.to_string().contains("opening-auction"));
    assert!(!client.capabilities().auction);
}

#[test]
fn rejects_reversed_csd_dates_instead_of_sorting_them() {
    let bridge = FakeBridge::new(REVERSED_BARS_BRIDGE_SCRIPT);
    let client = EmQuantClient::new(bridge.path()).unwrap();
    let request = BarsRequest::new(instruments()[0].clone(), BarInterval::Day, 2)
        .unwrap()
        .with_range("2026-07-20", "2026-07-22")
        .unwrap();

    let error = client.historical_bars(&request).unwrap_err();
    assert!(error.to_string().contains("duplicated or out of order"));
}

#[test]
fn publishes_independent_immutable_fake_bridges_in_parallel() {
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let paths = std::thread::scope(|scope| {
        let handles = (0..8)
            .map(|_| {
                let barrier = std::sync::Arc::clone(&barrier);
                scope.spawn(move || {
                    let bridge = fake_bridge();
                    let path = bridge.path().to_owned();
                    barrier.wait();
                    assert_eq!(
                        fs::read_to_string(bridge.path()).expect("read published fake bridge"),
                        DEFAULT_BRIDGE_SCRIPT
                    );
                    assert_ne!(
                        fs::metadata(bridge.path())
                            .expect("published fake bridge metadata")
                            .permissions()
                            .mode()
                            & 0o111,
                        0
                    );
                    path
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("parallel fake bridge thread"))
            .collect::<Vec<_>>()
    });
    let unique_paths = paths.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(unique_paths.len(), paths.len());
}
