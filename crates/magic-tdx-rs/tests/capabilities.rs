#[test]
fn tdx_advertises_all_core_data_families() {
    let c = magic_tdx_rs::TdxHqClient::capabilities();
    assert!(
        c.quotes
            && c.bars
            && c.minute
            && c.trades
            && c.fundamentals
            && c.corporate_actions
            && c.blocks
    );
}
