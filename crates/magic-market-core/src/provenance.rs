use serde::{Deserialize, Serialize};
/// Source and retrieval timestamps for a batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source: String,
    pub source_at: Option<String>,
    pub fetched_at: String,
    /// Stable per-batch evidence identifier supplied by the provider facade.
    pub batch_id: Option<String>,
}
impl Provenance {
    pub fn new(source: impl Into<String>, fetched_at: impl Into<String>) -> Self {
        let source = source.into();
        let fetched_at = fetched_at.into();
        Self {
            batch_id: Some(format!("{source}:{fetched_at}")),
            source,
            source_at: None,
            fetched_at,
        }
    }
    pub fn with_source_at(mut self, v: impl Into<String>) -> Self {
        self.source_at = Some(v.into());
        self
    }
    /// Overrides the generated batch identifier with a provider-issued one.
    pub fn with_batch_id(mut self, v: impl Into<String>) -> Self {
        self.batch_id = Some(v.into());
        self
    }
}
