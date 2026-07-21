use crate::source::{Adjustment,BarCategory,Market};
/// Validated bars request.
pub struct PacketBuilder;
impl PacketBuilder{pub fn bars(_market:Market,code:&str,_category:BarCategory,start:u32,count:u32,_adjustment:Adjustment)->Result<Vec<u8>,crate::TdxError>{if code.len()!=6||!code.bytes().all(|b|b.is_ascii_digit())||count==0{return Err(crate::TdxError::new(crate::ErrorKind::InvalidRequest,"invalid bars request",Some("code"),None))}let mut out=Vec::with_capacity(14);out.extend_from_slice(code.as_bytes());out.extend_from_slice(&start.to_le_bytes());out.extend_from_slice(&count.to_le_bytes());Ok(out)}}
