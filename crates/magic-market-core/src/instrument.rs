use crate::CoreError;
use serde::{Deserialize, Serialize};
/// Trading venue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange {
    Shanghai,
    Shenzhen,
    Beijing,
}
/// Instrument category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetClass {
    Equity,
    Index,
    Fund,
    Bond,
}
/// Validated exchange instrument identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstrumentId {
    exchange: Exchange,
    code: String,
    asset_class: AssetClass,
}
impl InstrumentId {
    /// Constructs an identifier.
    pub fn new(
        exchange: Exchange,
        code: impl Into<String>,
        asset_class: AssetClass,
    ) -> Result<Self, CoreError> {
        let code = code.into().trim().to_owned();
        if code.is_empty() {
            return Err(CoreError::InvalidInstrument("empty code".into()));
        }
        Ok(Self {
            exchange,
            code,
            asset_class,
        })
    }
    /// Venue.
    pub fn exchange(&self) -> Exchange {
        self.exchange
    }
    /// Code.
    pub fn code(&self) -> &str {
        &self.code
    }
    /// Category.
    pub fn asset_class(&self) -> AssetClass {
        self.asset_class
    }
}
