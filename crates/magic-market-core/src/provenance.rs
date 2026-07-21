use serde::{Deserialize,Serialize};
/// Source and retrieval timestamps for a batch.
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)] pub struct Provenance{pub source:String,pub source_at:Option<String>,pub fetched_at:String}
impl Provenance{pub fn new(source:impl Into<String>,fetched_at:impl Into<String>)->Self{Self{source:source.into(),source_at:None,fetched_at:fetched_at.into()}} pub fn with_source_at(mut self,v:impl Into<String>)->Self{self.source_at=Some(v.into());self}}
