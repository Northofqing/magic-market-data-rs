mod cursor; pub use cursor::ByteCursor;
/// Decode limits.
#[derive(Debug, Clone, Copy)] pub struct Limits { max_decompressed_bytes:usize }
impl Limits { pub fn builder()->LimitsBuilder{LimitsBuilder{max:16*1024*1024}} pub fn max_decompressed_bytes(&self)->usize{self.max_decompressed_bytes} }
pub struct LimitsBuilder{max:usize} impl LimitsBuilder { pub fn max_decompressed_bytes(mut self,v:usize)->Self{self.max=v;self} pub fn build(self)->Result<Limits,crate::TdxError>{if self.max==0{Err(crate::TdxError::new(crate::ErrorKind::InvalidRequest,"limit",None,None))}else{Ok(Limits{max_decompressed_bytes:self.max})}} }
/// Bounded passthrough decompression placeholder.
pub fn decompress_zlib(input:&[u8], limits:&Limits)->Result<Vec<u8>,crate::TdxError>{if input.len()>limits.max_decompressed_bytes(){Err(crate::TdxError::new(crate::ErrorKind::Decompression,"output limit",None,None))}else{Ok(input.to_vec())}}
