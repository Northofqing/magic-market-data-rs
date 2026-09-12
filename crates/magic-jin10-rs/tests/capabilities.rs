use magic_jin10_rs::{
    Jin10Client, ECONOMIC_CALENDAR_ADMITTED, ECONOMIC_RELEASE_OBSERVATIONS_ADMITTED,
};
use magic_market_core::{EconomicCalendarProvider, EconomicReleaseObservationsProvider};

fn assert_economic_calendar<T: EconomicCalendarProvider>() {}
fn assert_economic_release_observations<T: EconomicReleaseObservationsProvider>() {}

#[test]
fn advertises_public_news_and_narrow_release_observations() {
    assert_economic_calendar::<Jin10Client>();
    assert_economic_release_observations::<Jin10Client>();
    let capabilities = Jin10Client::content_capabilities();
    assert!(capabilities.global_news);
    assert!(!capabilities.instrument_news);
    assert!(!capabilities.announcements);
    assert!(!capabilities.market_announcements);
    assert!(!capabilities.investor_questions);
    let calendar = Jin10Client::calendar_capabilities();
    assert!(calendar.economic_releases);
    assert!(!calendar.futures_delivery);
    const { assert!(!ECONOMIC_CALENDAR_ADMITTED) };
    const { assert!(ECONOMIC_RELEASE_OBSERVATIONS_ADMITTED) };
}
