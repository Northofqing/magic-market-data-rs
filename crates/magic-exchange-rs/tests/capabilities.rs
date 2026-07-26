use magic_exchange_rs::{CffexClient, HkexClient, SseClient, SzseClient};
use magic_market_core::{FuturesDeliveryCalendar, ProviderId};

fn assert_delivery_calendar<T: FuturesDeliveryCalendar>() {}

#[test]
fn provider_identities_are_exact_and_unimplemented_families_remain_false() {
    assert_delivery_calendar::<CffexClient>();
    let sse = SseClient::capabilities();
    assert_eq!(sse.provider, ProviderId::Sse);
    assert_eq!(sse.market, Default::default());
    assert!(sse.content.announcements);
    assert!(!sse.content.instrument_news);
    assert!(!sse.content.global_news);
    assert!(!sse.content.investor_questions);
    assert_eq!(sse.capital, Default::default());
    assert!(sse.signals.dragon_tiger);

    let szse = SzseClient::capabilities();
    assert_eq!(szse.provider, ProviderId::Szse);
    assert!(szse.market.quotes);
    assert!(szse.market.order_book);
    assert!(!szse.market.auction);
    assert!(!szse.market.trades);
    assert!(szse.content.announcements);
    assert!(!szse.content.instrument_news);
    assert!(!szse.content.global_news);
    assert!(!szse.content.investor_questions);
    assert_eq!(szse.capital, Default::default());
    assert!(szse.signals.dragon_tiger);

    let hkex = HkexClient::capabilities();
    assert_eq!(hkex.provider, ProviderId::Hkex);
    assert_eq!(hkex.market, Default::default());
    assert_eq!(hkex.content, Default::default());
    assert!(hkex.capital.northbound_daily_statistics);
    assert_eq!(hkex.signals, Default::default());

    assert_eq!(CffexClient::provider_id(), ProviderId::Cffex);
    let calendar = CffexClient::calendar_capabilities();
    assert!(!calendar.futures_delivery);
    assert!(!calendar.economic_releases);
}
