use crate::{
    DataBatch, FiniteNumber, InstrumentId, IsoDate, Money, NonEmptyText, Price, Quantity, Ratio,
    SourceEvidence, SourcedRecord,
};
use serde::{de, Deserialize, Deserializer, Serialize};

/// Exchange contract month in canonical `YYYY-MM` form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ContractMonth(String);

impl ContractMonth {
    pub fn new(value: impl Into<String>) -> Result<Self, crate::CoreError> {
        let value = value.into();
        if value.len() != 7
            || value.as_bytes().get(4) != Some(&b'-')
            || !value
                .bytes()
                .enumerate()
                .all(|(index, byte)| index == 4 || byte.is_ascii_digit())
        {
            return Err(crate::CoreError::InvalidValue {
                field: "contract_month",
                value,
                reason: "must use YYYY-MM",
            });
        }
        IsoDate::new(format!("{value}-01"))?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ContractMonth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionKind {
    Call,
    Put,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OptionContractInput")]
#[non_exhaustive]
pub struct OptionContract {
    contract_code: NonEmptyText,
    underlying: InstrumentId,
    expiry_month: ContractMonth,
    expiry: Option<IsoDate>,
    kind: OptionKind,
    strike: Option<Price>,
    evidence: SourceEvidence,
}

impl OptionContract {
    pub fn new(input: OptionContractInput) -> Result<Self, crate::CoreError> {
        Self {
            contract_code: input.contract_code,
            underlying: input.underlying,
            expiry_month: input.expiry_month,
            expiry: input.expiry,
            kind: input.kind,
            strike: input.strike,
            evidence: input.evidence,
        }
        .validate()
    }

    pub fn contract_code(&self) -> &NonEmptyText {
        &self.contract_code
    }
    pub fn underlying(&self) -> &InstrumentId {
        &self.underlying
    }
    pub fn expiry_month(&self) -> &ContractMonth {
        &self.expiry_month
    }
    pub fn expiry(&self) -> Option<&IsoDate> {
        self.expiry.as_ref()
    }
    pub fn kind(&self) -> OptionKind {
        self.kind
    }
    pub fn strike(&self) -> Option<Price> {
        self.strike
    }
    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }

    fn validate(self) -> Result<Self, crate::CoreError> {
        if let Some(expiry) = self.expiry.as_ref() {
            if &expiry.as_str()[..7] != self.expiry_month.as_str() {
                return Err(crate::CoreError::InvalidRequest(format!(
                    "option expiry {} does not match contract month {}",
                    expiry,
                    self.expiry_month.as_str()
                )));
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptionContractInput {
    pub contract_code: NonEmptyText,
    pub underlying: InstrumentId,
    pub expiry_month: ContractMonth,
    pub expiry: Option<IsoDate>,
    pub kind: OptionKind,
    pub strike: Option<Price>,
    pub evidence: SourceEvidence,
}

impl TryFrom<OptionContractInput> for OptionContract {
    type Error = crate::CoreError;

    fn try_from(value: OptionContractInput) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OptionQuoteInput")]
#[non_exhaustive]
pub struct OptionQuote {
    contract_code: NonEmptyText,
    name: Option<NonEmptyText>,
    bid: Option<Price>,
    bid_quantity: Option<Quantity>,
    ask: Option<Price>,
    ask_quantity: Option<Quantity>,
    last: Option<Price>,
    previous_close: Option<Price>,
    open: Option<Price>,
    high: Option<Price>,
    low: Option<Price>,
    upper_limit: Option<Price>,
    lower_limit: Option<Price>,
    strike: Option<Price>,
    volume: Option<Quantity>,
    open_interest: Option<Quantity>,
    amount: Option<Money>,
    change: Option<Ratio>,
    amplitude: Option<Ratio>,
    quote_at: Option<NonEmptyText>,
    evidence: SourceEvidence,
}

fn validate_level(
    label: &str,
    price: Option<Price>,
    quantity: Option<Quantity>,
) -> Result<(), crate::CoreError> {
    if price.is_some() != quantity.is_some() {
        return Err(crate::CoreError::InvalidRequest(format!(
            "option {label} price and quantity must be present together"
        )));
    }
    Ok(())
}

fn valid_clock(value: &str) -> bool {
    value.len() == 8
        && value.as_bytes()[2] == b':'
        && value.as_bytes()[5] == b':'
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 2 || index == 5 || byte.is_ascii_digit())
        && value[0..2].parse::<u8>().is_ok_and(|hour| hour < 24)
        && value[3..5].parse::<u8>().is_ok_and(|minute| minute < 60)
        && value[6..8].parse::<u8>().is_ok_and(|second| second < 60)
}

fn valid_offset(value: &str) -> bool {
    value == "Z"
        || (value.len() == 6
            && matches!(value.as_bytes()[0], b'+' | b'-')
            && value.as_bytes()[3] == b':'
            && value[1..3].parse::<u8>().is_ok_and(|hour| hour <= 23)
            && value[4..6].parse::<u8>().is_ok_and(|minute| minute < 60))
}

fn valid_quote_timestamp(value: &str) -> bool {
    value.is_ascii()
        && value.len() >= 20
        && value.as_bytes()[10] == b'T'
        && IsoDate::new(&value[..10]).is_ok()
        && valid_clock(&value[11..19])
        && valid_offset(&value[19..])
}

impl OptionQuote {
    pub fn new(input: OptionQuoteInput) -> Result<Self, crate::CoreError> {
        Self {
            contract_code: input.contract_code,
            name: input.name,
            bid: input.bid,
            bid_quantity: input.bid_quantity,
            ask: input.ask,
            ask_quantity: input.ask_quantity,
            last: input.last,
            previous_close: input.previous_close,
            open: input.open,
            high: input.high,
            low: input.low,
            upper_limit: input.upper_limit,
            lower_limit: input.lower_limit,
            strike: input.strike,
            volume: input.volume,
            open_interest: input.open_interest,
            amount: input.amount,
            change: input.change,
            amplitude: input.amplitude,
            quote_at: input.quote_at,
            evidence: input.evidence,
        }
        .validate()
    }

    pub fn contract_code(&self) -> &NonEmptyText {
        &self.contract_code
    }
    pub fn name(&self) -> Option<&NonEmptyText> {
        self.name.as_ref()
    }
    pub fn bid(&self) -> Option<Price> {
        self.bid
    }
    pub fn bid_quantity(&self) -> Option<Quantity> {
        self.bid_quantity
    }
    pub fn ask(&self) -> Option<Price> {
        self.ask
    }
    pub fn ask_quantity(&self) -> Option<Quantity> {
        self.ask_quantity
    }
    pub fn last(&self) -> Option<Price> {
        self.last
    }
    pub fn previous_close(&self) -> Option<Price> {
        self.previous_close
    }
    pub fn open(&self) -> Option<Price> {
        self.open
    }
    pub fn high(&self) -> Option<Price> {
        self.high
    }
    pub fn low(&self) -> Option<Price> {
        self.low
    }
    pub fn upper_limit(&self) -> Option<Price> {
        self.upper_limit
    }
    pub fn lower_limit(&self) -> Option<Price> {
        self.lower_limit
    }
    pub fn strike(&self) -> Option<Price> {
        self.strike
    }
    pub fn volume(&self) -> Option<Quantity> {
        self.volume
    }
    pub fn open_interest(&self) -> Option<Quantity> {
        self.open_interest
    }
    pub fn amount(&self) -> Option<Money> {
        self.amount
    }
    pub fn change(&self) -> Option<Ratio> {
        self.change
    }
    pub fn amplitude(&self) -> Option<Ratio> {
        self.amplitude
    }
    pub fn quote_at(&self) -> Option<&NonEmptyText> {
        self.quote_at.as_ref()
    }
    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }

    fn validate(self) -> Result<Self, crate::CoreError> {
        validate_level("bid", self.bid, self.bid_quantity)?;
        validate_level("ask", self.ask, self.ask_quantity)?;
        for (left_name, left, right_name, right) in [
            ("bid", self.bid, "ask", self.ask),
            ("low", self.low, "high", self.high),
            (
                "lower limit",
                self.lower_limit,
                "upper limit",
                self.upper_limit,
            ),
        ] {
            if let (Some(left), Some(right)) = (left, right) {
                if left.get() > right.get() {
                    return Err(crate::CoreError::InvalidRequest(format!(
                        "option {left_name} must not exceed {right_name}"
                    )));
                }
            }
        }
        if self.amount.is_some_and(|amount| amount.get() < 0.0) {
            return Err(crate::CoreError::InvalidRequest(
                "option amount must be non-negative".into(),
            ));
        }
        if self
            .amplitude
            .is_some_and(|amplitude| amplitude.get() < 0.0)
        {
            return Err(crate::CoreError::InvalidRequest(
                "option amplitude must be non-negative".into(),
            ));
        }
        if self
            .quote_at
            .as_ref()
            .is_some_and(|value| !valid_quote_timestamp(value.as_str()))
        {
            return Err(crate::CoreError::InvalidRequest(
                "option quote_at must use a valid ISO-8601 second timestamp with offset".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptionQuoteInput {
    pub contract_code: NonEmptyText,
    pub name: Option<NonEmptyText>,
    pub bid: Option<Price>,
    pub bid_quantity: Option<Quantity>,
    pub ask: Option<Price>,
    pub ask_quantity: Option<Quantity>,
    pub last: Option<Price>,
    pub previous_close: Option<Price>,
    pub open: Option<Price>,
    pub high: Option<Price>,
    pub low: Option<Price>,
    pub upper_limit: Option<Price>,
    pub lower_limit: Option<Price>,
    pub strike: Option<Price>,
    pub volume: Option<Quantity>,
    pub open_interest: Option<Quantity>,
    pub amount: Option<Money>,
    pub change: Option<Ratio>,
    pub amplitude: Option<Ratio>,
    pub quote_at: Option<NonEmptyText>,
    pub evidence: SourceEvidence,
}

impl TryFrom<OptionQuoteInput> for OptionQuote {
    type Error = crate::CoreError;

    fn try_from(value: OptionQuoteInput) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OptionGreeksInput")]
#[non_exhaustive]
pub struct OptionGreeks {
    contract_code: NonEmptyText,
    name: Option<NonEmptyText>,
    volume: Option<Quantity>,
    delta: Option<FiniteNumber>,
    gamma: Option<FiniteNumber>,
    theta: Option<FiniteNumber>,
    vega: Option<FiniteNumber>,
    rho: Option<FiniteNumber>,
    implied_volatility: Option<FiniteNumber>,
    high: Option<Price>,
    low: Option<Price>,
    trade_code: Option<NonEmptyText>,
    strike: Option<Price>,
    last: Option<Price>,
    theoretical_price: Option<Price>,
    evidence: SourceEvidence,
}

impl OptionGreeks {
    pub fn new(input: OptionGreeksInput) -> Result<Self, crate::CoreError> {
        Self {
            contract_code: input.contract_code,
            name: input.name,
            volume: input.volume,
            delta: input.delta,
            gamma: input.gamma,
            theta: input.theta,
            vega: input.vega,
            rho: input.rho,
            implied_volatility: input.implied_volatility,
            high: input.high,
            low: input.low,
            trade_code: input.trade_code,
            strike: input.strike,
            last: input.last,
            theoretical_price: input.theoretical_price,
            evidence: input.evidence,
        }
        .validate()
    }

    pub fn contract_code(&self) -> &NonEmptyText {
        &self.contract_code
    }
    pub fn name(&self) -> Option<&NonEmptyText> {
        self.name.as_ref()
    }
    pub fn volume(&self) -> Option<Quantity> {
        self.volume
    }
    pub fn delta(&self) -> Option<FiniteNumber> {
        self.delta
    }
    pub fn gamma(&self) -> Option<FiniteNumber> {
        self.gamma
    }
    pub fn theta(&self) -> Option<FiniteNumber> {
        self.theta
    }
    pub fn vega(&self) -> Option<FiniteNumber> {
        self.vega
    }
    pub fn rho(&self) -> Option<FiniteNumber> {
        self.rho
    }
    pub fn implied_volatility(&self) -> Option<FiniteNumber> {
        self.implied_volatility
    }
    pub fn high(&self) -> Option<Price> {
        self.high
    }
    pub fn low(&self) -> Option<Price> {
        self.low
    }
    pub fn trade_code(&self) -> Option<&NonEmptyText> {
        self.trade_code.as_ref()
    }
    pub fn strike(&self) -> Option<Price> {
        self.strike
    }
    pub fn last(&self) -> Option<Price> {
        self.last
    }
    pub fn theoretical_price(&self) -> Option<Price> {
        self.theoretical_price
    }
    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }

    fn validate(self) -> Result<Self, crate::CoreError> {
        if self
            .delta
            .is_some_and(|value| !(-1.0..=1.0).contains(&value.get()))
        {
            return Err(crate::CoreError::InvalidRequest(
                "option delta must be between -1 and 1".into(),
            ));
        }
        for (name, value) in [
            ("gamma", self.gamma),
            ("vega", self.vega),
            ("implied volatility", self.implied_volatility),
        ] {
            if value.is_some_and(|value| value.get() < 0.0) {
                return Err(crate::CoreError::InvalidRequest(format!(
                    "option {name} must be non-negative"
                )));
            }
        }
        if let (Some(low), Some(high)) = (self.low, self.high) {
            if low.get() > high.get() {
                return Err(crate::CoreError::InvalidRequest(
                    "option Greek low must not exceed high".into(),
                ));
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptionGreeksInput {
    pub contract_code: NonEmptyText,
    pub name: Option<NonEmptyText>,
    pub volume: Option<Quantity>,
    pub delta: Option<FiniteNumber>,
    pub gamma: Option<FiniteNumber>,
    pub theta: Option<FiniteNumber>,
    pub vega: Option<FiniteNumber>,
    pub rho: Option<FiniteNumber>,
    pub implied_volatility: Option<FiniteNumber>,
    pub high: Option<Price>,
    pub low: Option<Price>,
    pub trade_code: Option<NonEmptyText>,
    pub strike: Option<Price>,
    pub last: Option<Price>,
    pub theoretical_price: Option<Price>,
    pub evidence: SourceEvidence,
}

impl TryFrom<OptionGreeksInput> for OptionGreeks {
    type Error = crate::CoreError;

    fn try_from(value: OptionGreeksInput) -> Result<Self, Self::Error> {
        Self::new(value)
    }
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
        expiry: Option<&ContractMonth>,
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
