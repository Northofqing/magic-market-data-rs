use magic_market_core::{DataBatch,Provenance};
#[test] fn strict_batch_preserves_metadata(){let p=Provenance::new("fixture","now").with_source_at("source");let b=DataBatch::strict(vec![1,2],p.clone());assert_eq!(b.records(),&[1,2]);assert_eq!(b.provenance(),&p);assert!(b.quality().complete);}
