use magic_tdx_rs::{Adjustment,BarCategory,Market,PacketBuilder,ResponseHeader};
#[test] fn header_rejects_truncation(){assert!(ResponseHeader::decode(&[5,0,0,0,1]).is_err());}
#[test] fn packet_validates_code(){assert!(PacketBuilder::bars(Market::Shanghai,"600000",BarCategory::Day,0,10,Adjustment::None).is_ok());assert!(PacketBuilder::bars(Market::Shanghai,"bad",BarCategory::Day,0,10,Adjustment::None).is_err());}
