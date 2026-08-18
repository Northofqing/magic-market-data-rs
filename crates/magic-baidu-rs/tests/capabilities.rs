use magic_baidu_rs::{BaiduClient, TECHNICAL_BARS_ADMITTED};

const _: () = assert!(TECHNICAL_BARS_ADMITTED);

#[test]
fn technical_bars_are_admitted_without_promoting_generic_historical_bars() {
    let capabilities = BaiduClient::capabilities();
    assert!(!capabilities.bars);
    assert!(!capabilities.quotes);
    assert!(!capabilities.minute);
    assert!(!capabilities.trades);
}
