use std::fs;
#[test]
fn security_count_fixture_is_stable() {
    let bytes = fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/protocol/security_count_one.bin")).unwrap();
    assert_eq!(&bytes[..2], [1, 0]);
    assert_eq!(magic_tdx_rs::protocol::parsers::parse_security_count(&bytes).unwrap(), 1);
}
