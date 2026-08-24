use magic_jin10_rs::{Jin10Client, ECONOMIC_CALENDAR_ADMITTED};
use magic_market_core::EconomicCalendarProvider;

fn assert_economic_calendar<T: EconomicCalendarProvider>() {}

#[test]
fn advertises_only_verified_global_news() {
    assert_economic_calendar::<Jin10Client>();
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
}
