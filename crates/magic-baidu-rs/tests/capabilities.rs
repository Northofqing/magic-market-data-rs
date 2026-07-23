use magic_baidu_rs::BaiduClient;

#[test]
fn advertises_only_verified_daily_bars() {
    let capabilities = BaiduClient::capabilities();
    assert!(capabilities.bars);
    assert!(!capabilities.quotes);
    assert!(!capabilities.minute);
    assert!(!capabilities.trades);
}
