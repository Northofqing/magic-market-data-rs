use magic_pbc_rs::{
    PbcClient, MONEY_SUPPLY_ADMITTED, REGIONAL_SERIES_ADMITTED, SOCIAL_FINANCING_ADMITTED,
};

#[test]
fn only_the_audited_table_family_can_be_admitted() {
    let capabilities = PbcClient::economic_data_capabilities();
    assert_eq!(capabilities.economic_series, MONEY_SUPPLY_ADMITTED);
    assert_eq!(SOCIAL_FINANCING_ADMITTED, capabilities.regional_series);
    assert_eq!(REGIONAL_SERIES_ADMITTED, capabilities.regional_series);
}
