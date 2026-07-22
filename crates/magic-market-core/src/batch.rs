use crate::Provenance;
use serde::{Deserialize, Serialize};
/// Quality state attached to returned records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QualityReport {
    pub complete: bool,
    pub issues: Vec<String>,
}
/// Records plus provenance and quality metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataBatch<T> {
    records: Vec<T>,
    provenance: Provenance,
    quality: QualityReport,
}
impl<T> DataBatch<T> {
    pub fn strict(records: Vec<T>, provenance: Provenance) -> Self {
        Self {
            records,
            provenance,
            quality: QualityReport {
                complete: true,
                issues: Vec::new(),
            },
        }
    }
    pub fn records(&self) -> &[T] {
        &self.records
    }
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }
    pub fn quality(&self) -> &QualityReport {
        &self.quality
    }
    pub fn into_records(self) -> Vec<T> {
        self.records
    }

    /// Constructs a batch whose completeness is explicitly reported.
    pub fn best_effort(records: Vec<T>, provenance: Provenance, issues: Vec<String>) -> Self {
        Self {
            records,
            provenance,
            quality: QualityReport {
                complete: issues.is_empty(),
                issues,
            },
        }
    }
}
