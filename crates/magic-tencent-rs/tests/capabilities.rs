use magic_market_core::{
    HistoricalBars, MinuteData, OrderBooks, RealtimeQuotes, SecurityMetadataProvider, Trades,
};
use magic_tencent_rs::{TencentClient, TencentError};

#[test]
fn public_client_implements_every_advertised_contract() {
    fn assert_contracts<
        T: RealtimeQuotes<Error = TencentError>
            + HistoricalBars<Error = TencentError>
            + MinuteData<Error = TencentError>
            + Trades<Error = TencentError>
            + OrderBooks<Error = TencentError>
            + SecurityMetadataProvider<Error = TencentError>,
    >() {
    }

    assert_contracts::<TencentClient>();
    let capabilities = TencentClient::capabilities();
    assert!(
        capabilities.quotes
            && capabilities.bars
            && capabilities.minute
            && capabilities.trades
            && capabilities.order_book
            && capabilities.security_metadata
    );
    assert!(
        !capabilities.fundamentals
            && !capabilities.corporate_actions
            && !capabilities.blocks
            && !capabilities.money_flow
            && !capabilities.auction
    );
}
