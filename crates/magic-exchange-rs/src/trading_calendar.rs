use crate::ExchangeError;
use magic_market_core::{
    CalendarCapabilities, DataBatch, IsoDate, Provenance, ProviderId, SourceEvidence,
    TradingCalendar, TradingDayEntry, TradingDayRequest, TradingVenue,
};
use std::time::{SystemTime, UNIX_EPOCH};

/// The formal production scope is a checked-in, revisioned 2026 Shanghai
/// trading-day vector. It performs no runtime network I/O, rejects every
/// other year and venue before any work, and is never extended by a runtime
/// calendar formula.
pub const SSE_2026_TRADING_CALENDAR_ADMITTED: bool = true;
const FIXED_TRADING_CALENDAR_YEAR: u32 = 2026;
/// Reviewed revision identity for the checked-in 2026 Shanghai vector.
pub const FIXED_TRADING_CALENDAR_REVISION: &str = "sse-trading-days-2026-v1";
/// Official SSE annual closure notice (上证公告〔2025〕45号).
pub const SSE_TRADING_CALENDAR_2026_NOTICE_URL: &str =
    "https://www.sse.com.cn/disclosure/announcement/general/c/c_20251222_10802507.shtml";
/// The SSE notice's own publication date; the family's `source_at` value.
pub const SSE_TRADING_CALENDAR_2026_NOTICE_SOURCE_AT: &str = "2025-12-22";
/// State Council 2026 holiday arrangement, already pinned by the CFFEX family.
pub const TRADING_CALENDAR_2026_HOLIDAY_NOTICE_URL: &str =
    "https://www.gov.cn/gongbao/2025/issue_12406/material/gwygb202532.pdf";

// All 2026 Mon-Fri dates minus the closure stretches announced by the SSE
// notice above: 01-01..01-04, 02-14..02-23, 04-04..04-06, 05-01..05-05,
// 06-19..06-21, 09-25..09-27 and 10-01..10-07. 242 entries; reviewed
// 2026-10-15 against the live exchange calendar. The runtime only reads this
// table; a new year requires a new reviewed revision.
const FIXED_TRADING_DAYS_2026: [&str; 242] = [
    "2026-01-05",
    "2026-01-06",
    "2026-01-07",
    "2026-01-08",
    "2026-01-09",
    "2026-01-12",
    "2026-01-13",
    "2026-01-14",
    "2026-01-15",
    "2026-01-16",
    "2026-01-19",
    "2026-01-20",
    "2026-01-21",
    "2026-01-22",
    "2026-01-23",
    "2026-01-26",
    "2026-01-27",
    "2026-01-28",
    "2026-01-29",
    "2026-01-30",
    "2026-02-02",
    "2026-02-03",
    "2026-02-04",
    "2026-02-05",
    "2026-02-06",
    "2026-02-09",
    "2026-02-10",
    "2026-02-11",
    "2026-02-12",
    "2026-02-13",
    "2026-02-24",
    "2026-02-25",
    "2026-02-26",
    "2026-02-27",
    "2026-03-02",
    "2026-03-03",
    "2026-03-04",
    "2026-03-05",
    "2026-03-06",
    "2026-03-09",
    "2026-03-10",
    "2026-03-11",
    "2026-03-12",
    "2026-03-13",
    "2026-03-16",
    "2026-03-17",
    "2026-03-18",
    "2026-03-19",
    "2026-03-20",
    "2026-03-23",
    "2026-03-24",
    "2026-03-25",
    "2026-03-26",
    "2026-03-27",
    "2026-03-30",
    "2026-03-31",
    "2026-04-01",
    "2026-04-02",
    "2026-04-03",
    "2026-04-07",
    "2026-04-08",
    "2026-04-09",
    "2026-04-10",
    "2026-04-13",
    "2026-04-14",
    "2026-04-15",
    "2026-04-16",
    "2026-04-17",
    "2026-04-20",
    "2026-04-21",
    "2026-04-22",
    "2026-04-23",
    "2026-04-24",
    "2026-04-27",
    "2026-04-28",
    "2026-04-29",
    "2026-04-30",
    "2026-05-06",
    "2026-05-07",
    "2026-05-08",
    "2026-05-11",
    "2026-05-12",
    "2026-05-13",
    "2026-05-14",
    "2026-05-15",
    "2026-05-18",
    "2026-05-19",
    "2026-05-20",
    "2026-05-21",
    "2026-05-22",
    "2026-05-25",
    "2026-05-26",
    "2026-05-27",
    "2026-05-28",
    "2026-05-29",
    "2026-06-01",
    "2026-06-02",
    "2026-06-03",
    "2026-06-04",
    "2026-06-05",
    "2026-06-08",
    "2026-06-09",
    "2026-06-10",
    "2026-06-11",
    "2026-06-12",
    "2026-06-15",
    "2026-06-16",
    "2026-06-17",
    "2026-06-18",
    "2026-06-22",
    "2026-06-23",
    "2026-06-24",
    "2026-06-25",
    "2026-06-26",
    "2026-06-29",
    "2026-06-30",
    "2026-07-01",
    "2026-07-02",
    "2026-07-03",
    "2026-07-06",
    "2026-07-07",
    "2026-07-08",
    "2026-07-09",
    "2026-07-10",
    "2026-07-13",
    "2026-07-14",
    "2026-07-15",
    "2026-07-16",
    "2026-07-17",
    "2026-07-20",
    "2026-07-21",
    "2026-07-22",
    "2026-07-23",
    "2026-07-24",
    "2026-07-27",
    "2026-07-28",
    "2026-07-29",
    "2026-07-30",
    "2026-07-31",
    "2026-08-03",
    "2026-08-04",
    "2026-08-05",
    "2026-08-06",
    "2026-08-07",
    "2026-08-10",
    "2026-08-11",
    "2026-08-12",
    "2026-08-13",
    "2026-08-14",
    "2026-08-17",
    "2026-08-18",
    "2026-08-19",
    "2026-08-20",
    "2026-08-21",
    "2026-08-24",
    "2026-08-25",
    "2026-08-26",
    "2026-08-27",
    "2026-08-28",
    "2026-08-31",
    "2026-09-01",
    "2026-09-02",
    "2026-09-03",
    "2026-09-04",
    "2026-09-07",
    "2026-09-08",
    "2026-09-09",
    "2026-09-10",
    "2026-09-11",
    "2026-09-14",
    "2026-09-15",
    "2026-09-16",
    "2026-09-17",
    "2026-09-18",
    "2026-09-21",
    "2026-09-22",
    "2026-09-23",
    "2026-09-24",
    "2026-09-28",
    "2026-09-29",
    "2026-09-30",
    "2026-10-08",
    "2026-10-09",
    "2026-10-12",
    "2026-10-13",
    "2026-10-14",
    "2026-10-15",
    "2026-10-16",
    "2026-10-19",
    "2026-10-20",
    "2026-10-21",
    "2026-10-22",
    "2026-10-23",
    "2026-10-26",
    "2026-10-27",
    "2026-10-28",
    "2026-10-29",
    "2026-10-30",
    "2026-11-02",
    "2026-11-03",
    "2026-11-04",
    "2026-11-05",
    "2026-11-06",
    "2026-11-09",
    "2026-11-10",
    "2026-11-11",
    "2026-11-12",
    "2026-11-13",
    "2026-11-16",
    "2026-11-17",
    "2026-11-18",
    "2026-11-19",
    "2026-11-20",
    "2026-11-23",
    "2026-11-24",
    "2026-11-25",
    "2026-11-26",
    "2026-11-27",
    "2026-11-30",
    "2026-12-01",
    "2026-12-02",
    "2026-12-03",
    "2026-12-04",
    "2026-12-07",
    "2026-12-08",
    "2026-12-09",
    "2026-12-10",
    "2026-12-11",
    "2026-12-14",
    "2026-12-15",
    "2026-12-16",
    "2026-12-17",
    "2026-12-18",
    "2026-12-21",
    "2026-12-22",
    "2026-12-23",
    "2026-12-24",
    "2026-12-25",
    "2026-12-28",
    "2026-12-29",
    "2026-12-30",
    "2026-12-31",
];

/// Checked-in Shanghai trading calendar (formal trait, zero transport).
///
/// This is a derived revision: the exchange publishes closure arrangements,
/// not a machine-readable trading-day vector. Every date traces to the SSE
/// notice and the State Council holiday arrangement; generation happened at
/// review time, never at runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct SseTradingCalendar;

impl SseTradingCalendar {
    pub const fn new() -> Self {
        Self
    }

    pub const fn provider_id() -> ProviderId {
        ProviderId::Sse
    }

    pub const fn calendar_capabilities() -> CalendarCapabilities {
        CalendarCapabilities {
            economic_releases: false,
            futures_delivery: false,
            trading_calendar: SSE_2026_TRADING_CALENDAR_ADMITTED,
        }
    }
}

impl TradingCalendar for SseTradingCalendar {
    type Error = ExchangeError;

    fn trading_calendar(
        &self,
        request: &TradingDayRequest,
    ) -> Result<DataBatch<TradingDayEntry>, Self::Error> {
        fixed_sse_trading_calendar(request)
    }
}

fn fixed_sse_trading_calendar(
    request: &TradingDayRequest,
) -> Result<DataBatch<TradingDayEntry>, ExchangeError> {
    if request.venue() != TradingVenue::Sse {
        return Err(ExchangeError::Unsupported(
            "formal trading calendar is admitted only for venue Sse; Shenzhen applicability is not established".into(),
        ));
    }
    if request.year().get() != FIXED_TRADING_CALENDAR_YEAR {
        return Err(ExchangeError::Unsupported(format!(
            "formal SSE trading calendar is admitted only for {FIXED_TRADING_CALENDAR_YEAR}; requested {}",
            request.year().get()
        )));
    }
    let observed_at = now()?;
    let batch_id = FIXED_TRADING_CALENDAR_REVISION.to_owned();
    let provenance = Provenance::new(FIXED_TRADING_CALENDAR_REVISION, observed_at.clone())?
        .with_batch_id(batch_id.clone())?
        .with_source_at(SSE_TRADING_CALENDAR_2026_NOTICE_SOURCE_AT)?;
    let evidence = SourceEvidence::new(ProviderId::Sse, observed_at, batch_id)?
        .with_source_at(SSE_TRADING_CALENDAR_2026_NOTICE_SOURCE_AT)?;
    let records = FIXED_TRADING_DAYS_2026
        .iter()
        .map(|date| {
            Ok(TradingDayEntry {
                date: IsoDate::new(*date)?,
                venue: TradingVenue::Sse,
                evidence: evidence.clone(),
            })
        })
        .collect::<Result<Vec<_>, ExchangeError>>()?;
    Ok(DataBatch::strict(records, provenance))
}

fn now() -> Result<String, ExchangeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| ExchangeError::Transport(format!("system clock error: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use magic_market_core::{PositiveU32, SourcedRecord};

    /// Gregorian ordinal copied from the Core contract so the test derives
    /// weekdays independently of the sealed table.
    fn ordinal(date: &str) -> u32 {
        let year = date[0..4].parse::<u32>().unwrap();
        let month = date[5..7].parse::<u32>().unwrap();
        let day = date[8..10].parse::<u32>().unwrap();
        let completed_year = year - 1;
        let leap_days = completed_year / 4 - completed_year / 100 + completed_year / 400;
        let days_before_month =
            [0_u32, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][(month - 1) as usize];
        let leap_adjustment =
            u32::from(month > 2 && year % 4 == 0 && (year % 100 != 0 || year % 400 == 0));
        completed_year * 365 + leap_days + days_before_month + leap_adjustment + day
    }

    fn year_request(year: u32, venue: TradingVenue) -> TradingDayRequest {
        TradingDayRequest::new(PositiveU32::new(year).unwrap(), venue).unwrap()
    }

    #[test]
    fn sealed_vector_matches_the_announced_closure_structure() {
        assert_eq!(FIXED_TRADING_DAYS_2026.len(), 242);
        assert_eq!(FIXED_TRADING_DAYS_2026[0], "2026-01-05");
        assert_eq!(FIXED_TRADING_DAYS_2026[241], "2026-12-31");
        // Strictly increasing and unique.
        for pair in FIXED_TRADING_DAYS_2026.windows(2) {
            assert!(pair[0] < pair[1], "not strictly increasing: {pair:?}");
        }
        // Every entry is a Monday..Friday; 2026-01-01 is a Thursday, so
        // Monday is (ordinal(date) - ordinal(origin) + 3) % 7 == 0.
        let origin = ordinal("2026-01-01");
        for date in FIXED_TRADING_DAYS_2026 {
            let weekday = (ordinal(date) - origin + 3) % 7;
            assert!(weekday < 5, "{date} falls on a weekend");
        }
        // No entry inside any announced closure stretch.
        let closures = [
            ("2026-01-01", "2026-01-04"),
            ("2026-02-14", "2026-02-23"),
            ("2026-04-04", "2026-04-06"),
            ("2026-05-01", "2026-05-05"),
            ("2026-06-19", "2026-06-21"),
            ("2026-09-25", "2026-09-27"),
            ("2026-10-01", "2026-10-07"),
        ];
        for (start, end) in closures {
            let position = FIXED_TRADING_DAYS_2026
                .iter()
                .position(|date| *date >= start)
                .unwrap();
            assert!(
                FIXED_TRADING_DAYS_2026[position] > end,
                "closure {start}..={end} contains {}",
                FIXED_TRADING_DAYS_2026[position]
            );
        }
        // Resume and pre-closure boundary days are present.
        for boundary in [
            "2026-01-05",
            "2026-02-13",
            "2026-02-24",
            "2026-04-03",
            "2026-04-07",
            "2026-04-30",
            "2026-05-06",
            "2026-06-18",
            "2026-06-22",
            "2026-09-24",
            "2026-09-28",
            "2026-09-30",
            "2026-10-08",
        ] {
            assert!(
                FIXED_TRADING_DAYS_2026.contains(&boundary),
                "boundary day {boundary} missing"
            );
        }
    }

    #[test]
    fn runtime_never_extends_or_reinterprets_other_years() {
        for year in [2025_u32, 2027, 2030] {
            let error = SseTradingCalendar::new()
                .trading_calendar(&year_request(year, TradingVenue::Sse))
                .unwrap_err();
            assert!(
                matches!(&error, ExchangeError::Unsupported(reason) if reason.contains("2026")),
                "year {year} must be Unsupported before any work: {error:?}"
            );
        }
    }

    #[test]
    fn shenzhen_venue_is_rejected_explicitly() {
        let error = SseTradingCalendar::new()
            .trading_calendar(&year_request(2026, TradingVenue::Szse))
            .unwrap_err();
        assert!(
            matches!(&error, ExchangeError::Unsupported(reason) if reason.contains("Sse")),
            "Szse must be Unsupported: {error:?}"
        );
    }

    #[test]
    fn admitted_year_returns_the_full_revision_with_notice_evidence() {
        let batch = SseTradingCalendar::new()
            .trading_calendar(&year_request(2026, TradingVenue::Sse))
            .unwrap();
        assert!(batch.quality().is_complete());
        assert_eq!(batch.records().len(), 242);
        assert_eq!(
            batch.provenance().batch_id(),
            Some(FIXED_TRADING_CALENDAR_REVISION)
        );
        assert_eq!(
            batch.provenance().source_at(),
            Some(SSE_TRADING_CALENDAR_2026_NOTICE_SOURCE_AT)
        );
        for record in batch.records() {
            assert_eq!(record.venue, TradingVenue::Sse);
            assert_eq!(record.evidence.provider(), ProviderId::Sse);
            assert_eq!(
                record.evidence.source_at(),
                Some(SSE_TRADING_CALENDAR_2026_NOTICE_SOURCE_AT)
            );
            assert_eq!(record.evidence.batch_id(), FIXED_TRADING_CALENDAR_REVISION);
            assert_eq!(
                record.evidence_observed_at(),
                Some(record.evidence.observed_at())
            );
        }
    }

    #[test]
    fn request_wire_shape_is_exact_and_rejects_unknown_venues() {
        let request: TradingDayRequest =
            serde_json::from_str(r#"{"year":2026,"venue":"Sse"}"#).unwrap();
        assert_eq!(request.year().get(), 2026);
        assert_eq!(request.venue(), TradingVenue::Sse);
        assert!(
            serde_json::from_str::<TradingDayRequest>(r#"{"year":2026,"venue":"SseStock"}"#)
                .is_err()
        );
    }
}
