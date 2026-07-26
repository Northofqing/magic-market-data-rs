use magic_market_core::{AssetClass, Capabilities, ProviderId};
#[test]
fn provider_capabilities_are_explicit() {
    assert_eq!(ProviderId::Tdx, ProviderId::Tdx);
    assert_ne!(ProviderId::LocalTerminal, ProviderId::Tdx);
    assert!(!Capabilities::new().quotes);
}

#[test]
fn intelligence_sources_have_first_class_identities() {
    let providers = [
        ProviderId::Baidu,
        ProviderId::Tonghuashun,
        ProviderId::Iwencai,
        ProviderId::Cninfo,
        ProviderId::Cailianpress,
        ProviderId::Jin10,
        ProviderId::ThePaper,
        ProviderId::Yonhap,
        ProviderId::WallstreetCn,
        ProviderId::Sse,
        ProviderId::Szse,
        ProviderId::Hkex,
        ProviderId::LocalAnalysis,
    ];
    assert_eq!(providers.len(), 13);
    assert_eq!(AssetClass::Option, AssetClass::Option);
}

#[test]
fn financial_news_provider_identity_names_are_stable() {
    assert_eq!(
        serde_json::to_string(&ProviderId::Jin10).unwrap(),
        "\"Jin10\""
    );
    assert_eq!(
        serde_json::to_string(&ProviderId::ThePaper).unwrap(),
        "\"ThePaper\""
    );
    assert_eq!(
        serde_json::to_string(&ProviderId::Yonhap).unwrap(),
        "\"Yonhap\""
    );
    assert_eq!(
        serde_json::to_string(&ProviderId::WallstreetCn).unwrap(),
        "\"WallstreetCn\""
    );
}
