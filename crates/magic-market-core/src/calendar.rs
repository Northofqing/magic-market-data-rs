use crate::{
    DataBatch, HttpsUrl, IsoDate, NonEmptyText, PositiveU32, SourceEvidence, SourcedRecord,
};
use serde::{de, Deserialize, Deserializer, Serialize};

/// Bounded latest economic-release request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EconomicCalendarRequest {
    limit: PositiveU32,
    country: Option<NonEmptyText>,
}

impl EconomicCalendarRequest {
    pub fn new(limit: PositiveU32) -> Result<Self, crate::CoreError> {
        if limit.get() > 20 {
            return Err(crate::CoreError::InvalidRequest(
                "economic calendar limit must be at most 20".into(),
            ));
        }
        Ok(Self {
            limit,
            country: None,
        })
    }

    pub fn with_country(mut self, country: impl Into<String>) -> Result<Self, crate::CoreError> {
        self.country = Some(NonEmptyText::new(country)?);
        Ok(self)
    }

    pub fn limit(&self) -> PositiveU32 {
        self.limit
    }

    pub fn country(&self) -> Option<&NonEmptyText> {
        self.country.as_ref()
    }
}

impl<'de> Deserialize<'de> for EconomicCalendarRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            limit: PositiveU32,
            country: Option<String>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let mut request = Self::new(wire.limit).map_err(de::Error::custom)?;
        if let Some(country) = wire.country {
            request = request.with_country(country).map_err(de::Error::custom)?;
        }
        Ok(request)
    }
}

/// One scheduled or released macroeconomic indicator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomicEvent {
    pub event_id: NonEmptyText,
    pub indicator_id: PositiveU32,
    pub country: NonEmptyText,
    pub name: NonEmptyText,
    pub period: Option<NonEmptyText>,
    pub scheduled_at: NonEmptyText,
    pub released_at: NonEmptyText,
    pub previous: Option<NonEmptyText>,
    pub consensus: Option<NonEmptyText>,
    pub actual: Option<NonEmptyText>,
    pub revised: Option<NonEmptyText>,
    pub unit: Option<NonEmptyText>,
    pub importance: PositiveU32,
    pub impact: Option<NonEmptyText>,
    pub evidence: SourceEvidence,
}

impl SourcedRecord for EconomicEvent {
    fn provider_id(&self) -> crate::ProviderId {
        self.evidence.provider()
    }

    fn evidence_batch_id(&self) -> &str {
        self.evidence.batch_id()
    }
}

pub trait EconomicCalendarProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn economic_calendar(
        &self,
        request: &EconomicCalendarRequest,
    ) -> Result<DataBatch<EconomicEvent>, Self::Error>;
}

/// Bounded observations from a Provider's current public economic-release
/// window. This request deliberately has no date range: it cannot assert
/// calendar completeness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EconomicReleaseObservationsRequest {
    limit: PositiveU32,
    country: Option<NonEmptyText>,
}

impl EconomicReleaseObservationsRequest {
    pub fn new(limit: PositiveU32) -> Result<Self, crate::CoreError> {
        if limit.get() > 20 {
            return Err(crate::CoreError::InvalidRequest(
                "economic release observations limit must be at most 20".into(),
            ));
        }
        Ok(Self {
            limit,
            country: None,
        })
    }

    pub fn with_country(mut self, country: impl Into<String>) -> Result<Self, crate::CoreError> {
        self.country = Some(NonEmptyText::new(country)?);
        Ok(self)
    }

    pub fn limit(&self) -> PositiveU32 {
        self.limit
    }

    pub fn country(&self) -> Option<&NonEmptyText> {
        self.country.as_ref()
    }
}

impl<'de> Deserialize<'de> for EconomicReleaseObservationsRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            limit: PositiveU32,
            country: Option<String>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let mut request = Self::new(wire.limit).map_err(de::Error::custom)?;
        if let Some(country) = wire.country {
            request = request.with_country(country).map_err(de::Error::custom)?;
        }
        Ok(request)
    }
}

/// A structured, already-published release observed in a bounded Provider
/// window. It is not proof of a complete calendar.
pub type EconomicReleaseObservation = EconomicEvent;

pub trait EconomicReleaseObservationsProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn economic_release_observations(
        &self,
        request: &EconomicReleaseObservationsRequest,
    ) -> Result<DataBatch<EconomicReleaseObservation>, Self::Error>;
}

/// Inclusive date range for one Provider's published economic-release
/// schedule. This is intentionally distinct from a complete economic calendar
/// and from already-released observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EconomicReleaseScheduleRequest {
    start: IsoDate,
    end: IsoDate,
    limit: PositiveU32,
}

impl EconomicReleaseScheduleRequest {
    pub fn new(start: IsoDate, end: IsoDate, limit: PositiveU32) -> Result<Self, crate::CoreError> {
        if limit.get() > 100 {
            return Err(crate::CoreError::InvalidRequest(
                "economic release schedule limit must be at most 100".into(),
            ));
        }
        let start_day = gregorian_ordinal(&start);
        let end_day = gregorian_ordinal(&end);
        if start_day > end_day {
            return Err(crate::CoreError::InvalidRequest(
                "economic release schedule start must not be after end".into(),
            ));
        }
        if end_day - start_day >= 366 {
            return Err(crate::CoreError::InvalidRequest(
                "economic release schedule range must contain at most 366 days".into(),
            ));
        }
        Ok(Self { start, end, limit })
    }

    pub fn start(&self) -> &IsoDate {
        &self.start
    }

    pub fn end(&self) -> &IsoDate {
        &self.end
    }

    pub fn limit(&self) -> PositiveU32 {
        self.limit
    }
}

impl<'de> Deserialize<'de> for EconomicReleaseScheduleRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            start: IsoDate,
            end: IsoDate,
            limit: PositiveU32,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.start, wire.end, wire.limit).map_err(de::Error::custom)
    }
}

fn gregorian_ordinal(date: &IsoDate) -> u32 {
    let value = date.as_str();
    let year = value[0..4].parse::<u32>().expect("validated ISO year");
    let month = value[5..7].parse::<u32>().expect("validated ISO month");
    let day = value[8..10].parse::<u32>().expect("validated ISO day");
    let completed_year = year - 1;
    let leap_days = completed_year / 4 - completed_year / 100 + completed_year / 400;
    let days_before_month =
        [0_u32, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][(month - 1) as usize];
    let leap_adjustment = u32::from(
        month > 2
            && year.is_multiple_of(4)
            && (!year.is_multiple_of(100) || year.is_multiple_of(400)),
    );
    completed_year * 365 + leap_days + days_before_month + leap_adjustment + day
}

/// One date-only economic release entry published by a Provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EconomicReleaseScheduleEntry {
    release_id: PositiveU32,
    release_name: NonEmptyText,
    release_date: IsoDate,
    release_last_updated: Option<NonEmptyText>,
    evidence: SourceEvidence,
}

impl EconomicReleaseScheduleEntry {
    pub fn new(
        release_id: PositiveU32,
        release_name: impl Into<String>,
        release_date: IsoDate,
        release_last_updated: Option<String>,
        evidence: SourceEvidence,
    ) -> Result<Self, crate::CoreError> {
        if evidence.source_at().is_some() {
            return Err(crate::CoreError::InvalidRequest(
                "economic release schedule evidence must not contain source_at".into(),
            ));
        }
        Ok(Self {
            release_id,
            release_name: NonEmptyText::new(release_name)?,
            release_date,
            release_last_updated: release_last_updated.map(NonEmptyText::new).transpose()?,
            evidence,
        })
    }

    pub fn release_id(&self) -> PositiveU32 {
        self.release_id
    }

    pub fn release_name(&self) -> &NonEmptyText {
        &self.release_name
    }

    pub fn release_date(&self) -> &IsoDate {
        &self.release_date
    }

    pub fn release_last_updated(&self) -> Option<&NonEmptyText> {
        self.release_last_updated.as_ref()
    }

    pub fn evidence(&self) -> &SourceEvidence {
        &self.evidence
    }
}

impl<'de> Deserialize<'de> for EconomicReleaseScheduleEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            release_id: PositiveU32,
            release_name: String,
            release_date: IsoDate,
            release_last_updated: Option<String>,
            evidence: SourceEvidence,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(
            wire.release_id,
            wire.release_name,
            wire.release_date,
            wire.release_last_updated,
            wire.evidence,
        )
        .map_err(de::Error::custom)
    }
}

impl SourcedRecord for EconomicReleaseScheduleEntry {
    fn provider_id(&self) -> crate::ProviderId {
        self.evidence.provider()
    }

    fn evidence_batch_id(&self) -> &str {
        self.evidence.batch_id()
    }

    fn evidence_source_at(&self) -> Option<&str> {
        self.evidence.source_at()
    }

    fn evidence_observed_at(&self) -> Option<&str> {
        Some(self.evidence.observed_at())
    }
}

pub trait EconomicReleaseScheduleProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn economic_release_schedule(
        &self,
        request: &EconomicReleaseScheduleRequest,
    ) -> Result<DataBatch<EconomicReleaseScheduleEntry>, Self::Error>;
}

/// CFFEX equity-index-futures products admitted by the delivery-notice parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FuturesProduct {
    If,
    Ih,
    Ic,
    Im,
}

/// Request for one CFFEX contract month's official delivery notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FuturesDeliveryRequest {
    year: PositiveU32,
    month: PositiveU32,
}

impl FuturesDeliveryRequest {
    pub fn new(year: PositiveU32, month: PositiveU32) -> Result<Self, crate::CoreError> {
        if !(2000..=9999).contains(&year.get()) {
            return Err(crate::CoreError::InvalidRequest(
                "futures delivery year must be in 2000..=9999".into(),
            ));
        }
        if month.get() > 12 {
            return Err(crate::CoreError::InvalidRequest(
                "futures delivery month must be in 1..=12".into(),
            ));
        }
        Ok(Self { year, month })
    }

    pub fn year(&self) -> PositiveU32 {
        self.year
    }

    pub fn month(&self) -> PositiveU32 {
        self.month
    }
}

impl<'de> Deserialize<'de> for FuturesDeliveryRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            year: PositiveU32,
            month: PositiveU32,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.year, wire.month).map_err(de::Error::custom)
    }
}

/// Delivery method supported by the normalized calendar contract.
///
/// `NotProvided` means the event source proves the delivery event and date but
/// does not independently prove the settlement method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FuturesDeliveryMethod {
    Cash,
    NotProvided,
}

/// One contract delivery event proved by an official CFFEX notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuturesDeliveryEvent {
    pub product: FuturesProduct,
    pub contract_code: NonEmptyText,
    /// Last trading date when the source explicitly proves it.
    ///
    /// An official delivery notice can prove the delivery date without
    /// independently proving the last trading date.
    pub last_trading_date: Option<IsoDate>,
    pub delivery_date: IsoDate,
    pub method: FuturesDeliveryMethod,
    pub notice_url: HttpsUrl,
    pub evidence: SourceEvidence,
}

impl SourcedRecord for FuturesDeliveryEvent {
    fn provider_id(&self) -> crate::ProviderId {
        self.evidence.provider()
    }

    fn evidence_batch_id(&self) -> &str {
        self.evidence.batch_id()
    }
}

pub trait FuturesDeliveryCalendar {
    type Error: std::error::Error + Send + Sync + 'static;

    fn futures_delivery_calendar(
        &self,
        request: &FuturesDeliveryRequest,
    ) -> Result<DataBatch<FuturesDeliveryEvent>, Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CalendarCapabilities {
    pub economic_releases: bool,
    pub futures_delivery: bool,
}

#[cfg(test)]
#[path = "../tests/internal/calendar_tests.rs"]
mod tests;
