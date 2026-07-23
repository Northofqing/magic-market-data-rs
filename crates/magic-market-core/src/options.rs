use crate::{
    DataBatch, FiniteNumber, InstrumentId, IsoDate, NonEmptyText, Price, Quantity, Ratio,
    SourceEvidence, SourcedRecord,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionKind {
    Call,
    Put,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionContract {
    pub contract_code: NonEmptyText,
    pub underlying: InstrumentId,
    pub expiry: IsoDate,
    pub kind: OptionKind,
    pub strike: Price,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionQuote {
    pub contract_code: NonEmptyText,
    pub bid: Option<Price>,
    pub ask: Option<Price>,
    pub last: Option<Price>,
    pub volume: Option<Quantity>,
    pub open_interest: Option<Quantity>,
    pub change: Option<Ratio>,
    pub quote_at: Option<NonEmptyText>,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionGreeks {
    pub contract_code: NonEmptyText,
    pub delta: Option<FiniteNumber>,
    pub gamma: Option<FiniteNumber>,
    pub theta: Option<FiniteNumber>,
    pub vega: Option<FiniteNumber>,
    pub rho: Option<FiniteNumber>,
    pub implied_volatility: Option<FiniteNumber>,
    pub evidence: SourceEvidence,
}

macro_rules! impl_sourced {
    ($($record:ty),+ $(,)?) => {
        $(
            impl SourcedRecord for $record {
                fn provider_id(&self) -> crate::ProviderId {
                    self.evidence.provider()
                }

                fn evidence_batch_id(&self) -> &str {
                    self.evidence.batch_id()
                }
            }
        )+
    };
}

impl_sourced!(OptionContract, OptionQuote, OptionGreeks);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OptionCapabilities {
    pub contract_discovery: bool,
    pub quotes: bool,
    pub greeks: bool,
}

pub trait OptionData {
    type Error: std::error::Error + Send + Sync + 'static;
    fn option_contracts(
        &self,
        underlying: &InstrumentId,
        expiry: Option<&IsoDate>,
    ) -> Result<DataBatch<OptionContract>, Self::Error>;
    fn option_quotes(
        &self,
        contracts: &[NonEmptyText],
    ) -> Result<DataBatch<OptionQuote>, Self::Error>;
    fn option_greeks(
        &self,
        contracts: &[NonEmptyText],
    ) -> Result<DataBatch<OptionGreeks>, Self::Error>;
}
