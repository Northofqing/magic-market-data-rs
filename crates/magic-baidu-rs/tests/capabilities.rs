use magic_baidu_rs::BaiduClient;

#[test]
fn technical_bars_stay_unadvertised_until_continuity_gates_are_proved() {
    let capabilities = BaiduClient::capabilities();
    assert!(!capabilities.bars);
    assert!(!capabilities.quotes);
    assert!(!capabilities.minute);
    assert!(!capabilities.trades);
}
