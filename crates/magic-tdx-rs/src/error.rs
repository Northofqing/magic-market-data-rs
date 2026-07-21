use std::fmt;
/// Broad failure family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum ErrorKind { Decode, Decompression, InvalidRequest, Unsupported }
/// Structured decode context.
#[derive(Debug, Clone, PartialEq, Eq)] pub struct ErrorContext { field: Option<String>, offset: Option<usize> }
impl ErrorContext { pub fn field(&self)->Option<&str>{self.field.as_deref()} pub fn offset(&self)->Option<usize>{self.offset} }
/// Driver error.
#[derive(Debug, Clone, PartialEq, Eq)] pub struct TdxError { kind: ErrorKind, message:String, context:ErrorContext }
impl TdxError { pub fn new(kind:ErrorKind,message:impl Into<String>,field:Option<&str>,offset:Option<usize>)->Self{Self{kind,message:message.into(),context:ErrorContext{field:field.map(str::to_owned),offset}}} pub fn kind(&self)->ErrorKind{self.kind} pub fn context(&self)->&ErrorContext{&self.context} }
impl fmt::Display for TdxError { fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{write!(f,"{:?}: {}",self.kind,self.message)} }
impl std::error::Error for TdxError {}
