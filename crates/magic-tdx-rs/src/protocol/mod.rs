mod packet; pub use packet::PacketBuilder;
/// Validated response header.
pub struct ResponseHeader{pub body_len:u32}
impl ResponseHeader{pub fn decode(bytes:&[u8])->Result<Self,crate::TdxError>{if bytes.len()<4{return Err(crate::TdxError::new(crate::ErrorKind::Decode,"header",None,Some(0)))}let n=u32::from_le_bytes([bytes[0],bytes[1],bytes[2],bytes[3]]) as usize;if n>bytes.len()-4{return Err(crate::TdxError::new(crate::ErrorKind::Decode,"body length",None,Some(0)))}Ok(Self{body_len:n as u32})}}
