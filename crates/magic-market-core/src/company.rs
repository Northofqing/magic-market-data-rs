use crate::{
    DataBatch, FiniteNumber, InstrumentId, IsoDate, NonEmptyText, SourceEvidence, SourcedRecord,
    UnverifiedSourceUnit,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileFact {
    pub key: NonEmptyText,
    pub source_label: NonEmptyText,
    pub value: NonEmptyText,
}

/// A provider-native share count whose physical unit has not been
/// independently verified (`UnverifiedSourceUnit` discipline).
///
/// Consumers must not interpret `source_value` as shares without external
/// verification: it must not be used for market capitalisation, turnover or
/// share-ratio computation, and it is not comparable across providers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnverifiedShareCount {
    pub source_value: FiniteNumber,
    pub unit: UnverifiedSourceUnit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecurityProfile {
    pub instrument: InstrumentId,
    pub name: NonEmptyText,
    pub industry: Option<NonEmptyText>,
    pub listed_on: Option<IsoDate>,
    pub total_shares: Option<UnverifiedShareCount>,
    pub floating_shares: Option<UnverifiedShareCount>,
    pub facts: Vec<ProfileFact>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatementKind {
    Balance,
    Income,
    CashFlow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinancialLine {
    pub key: NonEmptyText,
    pub source_label: NonEmptyText,
    pub value: Option<FiniteNumber>,
    pub unit: Option<NonEmptyText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinancialStatement {
    pub instrument: InstrumentId,
    pub kind: StatementKind,
    pub report_period: IsoDate,
    /// Provider-native fiscal-period label such as `Q1`, `H1`, `Q3` or `FY`.
    ///
    /// This preserves source period identity. It does not by itself assert
    /// whether a numeric line is cumulative or single-quarter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fiscal_period: Option<NonEmptyText>,
    pub announced_on: Option<IsoDate>,
    pub currency: Option<NonEmptyText>,
    pub lines: Vec<FinancialLine>,
    pub evidence: SourceEvidence,
}

impl SourcedRecord for SecurityProfile {
    fn provider_id(&self) -> crate::ProviderId {
        self.evidence.provider()
    }

    fn evidence_batch_id(&self) -> &str {
        self.evidence.batch_id()
    }
}

impl SourcedRecord for FinancialStatement {
    fn provider_id(&self) -> crate::ProviderId {
        self.evidence.provider()
    }

    fn evidence_batch_id(&self) -> &str {
        self.evidence.batch_id()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CompanyCapabilities {
    pub security_profile: bool,
    pub balance_sheet: bool,
    pub income_statement: bool,
    pub cash_flow_statement: bool,
}

pub trait SecurityProfiles {
    type Error: std::error::Error + Send + Sync + 'static;
    fn security_profiles(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityProfile>, Self::Error>;
}

pub trait FinancialStatements {
    type Error: std::error::Error + Send + Sync + 'static;
    fn financial_statements(
        &self,
        instruments: &[InstrumentId],
        kind: StatementKind,
    ) -> Result<DataBatch<FinancialStatement>, Self::Error>;
}
