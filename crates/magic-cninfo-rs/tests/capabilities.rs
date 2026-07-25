use magic_cninfo_rs::{CninfoClient, CninfoError};
use magic_market_core::{AnnouncementDiscovery, Announcements, InvestorQuestions};

fn assert_announcement_provider<T: Announcements<Error = CninfoError>>() {}
fn assert_question_provider<T: InvestorQuestions<Error = CninfoError>>() {}
fn assert_discovery_provider<T: AnnouncementDiscovery<Error = CninfoError>>() {}

#[test]
fn public_traits_and_capabilities_match_the_implementation() {
    assert_announcement_provider::<CninfoClient>();
    assert_question_provider::<CninfoClient>();
    assert_discovery_provider::<CninfoClient>();

    let capabilities = CninfoClient::capabilities();
    assert!(capabilities.announcements);
    assert!(capabilities.announcement_discovery);
    assert!(capabilities.investor_questions);
    assert!(!capabilities.instrument_news);
    assert!(!capabilities.global_news);
}
