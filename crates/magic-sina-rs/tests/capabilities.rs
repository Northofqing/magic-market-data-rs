use magic_market_core::{
    FinancialStatements, HistoricalBars, MinuteData, NewsProvider, OptionData, OrderBooks,
    RealtimeQuotes, SecurityMetadataProvider,
};
use magic_sina_rs::{SinaClient, SinaError};

#[test]
fn public_client_implements_every_advertised_contract() {
    fn assert_contracts<
        T: RealtimeQuotes<Error = SinaError>
            + HistoricalBars<Error = SinaError>
            + MinuteData<Error = SinaError>
            + OrderBooks<Error = SinaError>
            + SecurityMetadataProvider<Error = SinaError>
            + FinancialStatements<Error = SinaError>
            + OptionData<Error = SinaError>
            + NewsProvider<Error = SinaError>,
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
            && capabilities.fundamentals
    );
    let option_capabilities = SinaClient::option_capabilities();
    assert!(
        option_capabilities.contract_discovery
            && option_capabilities.quotes
            && option_capabilities.greeks
    );
    let content_capabilities = SinaClient::content_capabilities();
    assert!(content_capabilities.instrument_news);
    assert!(
        !content_capabilities.global_news
            && !content_capabilities.announcements
            && !content_capabilities.investor_questions
    );
    assert!(
        !capabilities.trades
            && !capabilities.corporate_actions
            && !capabilities.blocks
            && !capabilities.money_flow
            && !capabilities.auction
    );
}
