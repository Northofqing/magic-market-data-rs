use magic_market_core::{
    HistoricalBars, MinuteData, OrderBooks, RealtimeQuotes, SecurityMetadataProvider,
};
use magic_sina_rs::{SinaClient, SinaError};

#[test]
fn public_client_implements_every_advertised_contract() {
    fn assert_contracts<
        T: RealtimeQuotes<Error = SinaError>
            + HistoricalBars<Error = SinaError>
            + MinuteData<Error = SinaError>
            + OrderBooks<Error = SinaError>
            + SecurityMetadataProvider<Error = SinaError>,
    >() {
    }

    assert_contracts::<SinaClient>();
    let capabilities = SinaClient::capabilities();
    assert!(
        capabilities.quotes
            && capabilities.bars
            && capabilities.minute
            && capabilities.order_book
            && capabilities.security_metadata
    );
    assert!(
        !capabilities.trades
            && !capabilities.fundamentals
            && !capabilities.corporate_actions
            && !capabilities.blocks
            && !capabilities.money_flow
            && !capabilities.auction
    );
}
