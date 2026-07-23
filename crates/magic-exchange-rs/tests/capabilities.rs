use magic_exchange_rs::{HkexClient, SseClient, SzseClient};
use magic_market_core::ProviderId;

#[test]
fn provider_identities_are_exact_and_unimplemented_families_remain_false() {
    let sse = SseClient::capabilities();
    assert_eq!(sse.provider, ProviderId::Sse);
    assert!(sse.content.announcements);
    assert!(!sse.content.instrument_news);
    assert!(!sse.content.global_news);
    assert!(!sse.content.investor_questions);

    let szse = SzseClient::capabilities();
    assert_eq!(szse.provider, ProviderId::Szse);
    assert!(szse.content.announcements);
    assert!(!szse.content.instrument_news);
    assert!(!szse.content.global_news);
    assert!(!szse.content.investor_questions);

    let hkex = HkexClient::capabilities();
    assert_eq!(hkex.provider, ProviderId::Hkex);
    assert_eq!(hkex.content, Default::default());
}
