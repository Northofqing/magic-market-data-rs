use magic_pbc_rs::{
    PbcClient, MONEY_SUPPLY_ADMITTED, REGIONAL_SERIES_ADMITTED, SOCIAL_FINANCING_ADMITTED,
};

#[test]
fn audited_money_supply_and_regional_social_financing_families_are_admitted() {
    let capabilities = PbcClient::economic_data_capabilities();
    assert_eq!(capabilities.economic_series, MONEY_SUPPLY_ADMITTED);
    assert_eq!(SOCIAL_FINANCING_ADMITTED, capabilities.regional_series);
    assert_eq!(REGIONAL_SERIES_ADMITTED, capabilities.regional_series);
}
