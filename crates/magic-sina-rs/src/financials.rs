use crate::{now, validate_instruments, SinaClient, SinaError};
use magic_market_core::{
    CompanyCapabilities, DataBatch, FinancialLine, FinancialStatement, FinancialStatements,
    FiniteNumber, InstrumentId, IsoDate, NonEmptyText, ProviderId, SourceEvidence, StatementKind,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

const FINANCIAL_ENDPOINT: &str =
    "https://quotes.sina.cn/cn/api/openapi.php/CompanyFinanceService.getFinanceReport2022";
const DEFAULT_PERIODS: usize = 8;
const MAX_FINANCIAL_INSTRUMENTS: usize = 10;

#[derive(Debug, Deserialize)]
struct FinancialRoot {
    result: FinancialResult,
}

#[derive(Debug, Deserialize)]
struct FinancialResult {
    status: FinancialStatus,
    data: FinancialData,
}

#[derive(Debug, Deserialize)]
struct FinancialStatus {
    code: i64,
}

#[derive(Debug, Deserialize)]
struct FinancialData {
    report_list: HashMap<String, FinancialPeriod>,
}

#[derive(Debug, Deserialize)]
struct FinancialPeriod {
    #[serde(rename = "rCurrency")]
    currency: Option<String>,
    publish_date: Option<String>,
    data: Vec<FinancialRow>,
}

#[derive(Debug, Deserialize)]
struct FinancialRow {
    item_field: String,
    item_title: String,
    item_value: Value,
    item_source: Option<String>,
}

fn statement_source(kind: StatementKind) -> &'static str {
    match kind {
        StatementKind::Balance => "fzb",
        StatementKind::Income => "lrb",
        StatementKind::CashFlow => "llb",
    }
}

fn financial_url(symbol: &str, kind: StatementKind) -> String {
    format!(
        "{FINANCIAL_ENDPOINT}?paperCode={symbol}&source={}&type=0&page=1&num={DEFAULT_PERIODS}",
        statement_source(kind)
    )
}

fn compact_date(value: &str, field: &'static str) -> Result<IsoDate, SinaError> {
    if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SinaError::Protocol(format!(
            "{field} must use YYYYMMDD: {value:?}"
        )));
    }
    Ok(IsoDate::new(format!(
        "{}-{}-{}",
        &value[..4],
        &value[4..6],
        &value[6..]
    ))?)
}

fn optional_value(value: &Value, key: &str) -> Result<Option<FiniteNumber>, SinaError> {
    let parsed = match value {
        Value::Null => return Ok(None),
        Value::String(value) if value.trim().is_empty() => return Ok(None),
        Value::String(value) => value.parse::<f64>().map_err(|_| {
            SinaError::Protocol(format!("financial line {key} is not numeric: {value:?}"))
        })?,
        Value::Number(value) => value.as_f64().ok_or_else(|| {
            SinaError::Protocol(format!("financial line {key} is outside f64 range"))
        })?,
        _ => {
            return Err(SinaError::Protocol(format!(
                "financial line {key} value must be a string, number or null"
            )));
        }
    };
    if !parsed.is_finite() {
        return Err(SinaError::Protocol(format!(
            "financial line {key} must be finite"
        )));
    }
    Ok(Some(FiniteNumber::new(parsed)?))
}

fn parse_lines(
    rows: Vec<FinancialRow>,
    kind: StatementKind,
    period: &str,
) -> Result<(Vec<FinancialLine>, Vec<String>), SinaError> {
    let expected_source = statement_source(kind);
    let mut lines = Vec::with_capacity(rows.len());
    let mut seen = HashMap::with_capacity(rows.len());
    let mut issues = Vec::new();
    for row in rows {
        let source_key = row.item_field.trim();
        let empty_value = matches!(&row.item_value, Value::Null)
            || matches!(&row.item_value, Value::String(value) if value.trim().is_empty());
        if source_key.is_empty() && empty_value {
            continue;
        }
        if source_key.is_empty() {
            return Err(SinaError::Protocol(
                "financial value has no stable item_field".into(),
            ));
        }
        if !source_key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(SinaError::Protocol(format!(
                "financial item_field contains unsupported characters: {source_key:?}"
            )));
        }
        if row
            .item_source
            .as_deref()
            .is_some_and(|source| source != expected_source)
        {
            return Err(SinaError::Protocol(format!(
                "financial line {source_key} source contradicts requested {expected_source}"
            )));
        }
        let key = source_key.to_ascii_lowercase();
        let fingerprint = (
            row.item_title.clone(),
            row.item_value.clone(),
            row.item_source.clone(),
        );
        if let Some(previous) = seen.get(&key) {
            if previous == &fingerprint {
                issues.push(format!(
                    "{period}: Sina returned an identical duplicate financial item_field {source_key}; one normalized copy retained"
                ));
                continue;
            }
            return Err(SinaError::Protocol(format!(
                "conflicting duplicate financial item_field {source_key}"
            )));
        }
        seen.insert(key.clone(), fingerprint);
        lines.push(FinancialLine {
            key: NonEmptyText::new(key)?,
            source_label: NonEmptyText::new(row.item_title)?,
            value: optional_value(&row.item_value, source_key)?,
            unit: None,
        });
    }
    if lines.is_empty() {
        return Err(SinaError::Protocol(
            "financial period contains no keyed line items".into(),
        ));
    }
    Ok((lines, issues))
}

pub(crate) fn parse_financial_response(
    bytes: &[u8],
    instrument: &InstrumentId,
    kind: StatementKind,
    observed_at: &str,
) -> Result<DataBatch<FinancialStatement>, SinaError> {
    let root: FinancialRoot = serde_json::from_slice(bytes)
        .map_err(|error| SinaError::Decode(format!("financial JSON: {error}")))?;
    if root.result.status.code != 0 {
        return Err(SinaError::Protocol(format!(
            "financial endpoint returned status code {}",
            root.result.status.code
        )));
    }
    if root.result.data.report_list.is_empty() {
        return Err(SinaError::Protocol(
            "financial response report_list is empty".into(),
        ));
    }
    if root.result.data.report_list.len() > DEFAULT_PERIODS {
        return Err(SinaError::Protocol(format!(
            "financial endpoint returned {} periods for limit {DEFAULT_PERIODS}",
            root.result.data.report_list.len()
        )));
    }
    let mut periods = root.result.data.report_list.into_iter().collect::<Vec<_>>();
    periods.sort_unstable_by(|left, right| right.0.cmp(&left.0));

    let batch_id = format!("sina-web:{observed_at}:financial:{kind:?}");
    let mut records = Vec::with_capacity(periods.len());
    let mut issues = Vec::new();
    for (period, source) in periods {
        let report_period = compact_date(&period, "financial report period")?;
        let announced_on = source
            .publish_date
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| compact_date(value, "financial publish date"))
            .transpose()?;
        let mut evidence = SourceEvidence::new(ProviderId::Sina, observed_at, batch_id.clone())?;
        if let Some(date) = announced_on.as_ref() {
            evidence = evidence.with_source_at(date.as_str())?;
        }
        let currency = source
            .currency
            .filter(|value| !value.trim().is_empty())
            .map(NonEmptyText::new)
            .transpose()?;
        let (lines, line_issues) = parse_lines(source.data, kind, &period)?;
        issues.extend(line_issues);
        records.push(FinancialStatement {
            instrument: instrument.clone(),
            kind,
            report_period,
            announced_on,
            currency,
            lines,
            evidence,
        });
    }
    let provenance =
        magic_market_core::Provenance::new("sina-web", observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::best_effort(records, provenance, issues)?)
}

impl SinaClient {
    pub const fn company_capabilities() -> CompanyCapabilities {
        CompanyCapabilities {
            security_profile: false,
            balance_sheet: true,
            income_statement: true,
            cash_flow_statement: true,
        }
    }
}

impl FinancialStatements for SinaClient {
    type Error = SinaError;

    fn financial_statements(
        &self,
        instruments: &[InstrumentId],
        kind: StatementKind,
    ) -> Result<DataBatch<FinancialStatement>, Self::Error> {
        if instruments.len() > MAX_FINANCIAL_INSTRUMENTS {
            return Err(SinaError::InvalidRequest(format!(
                "Sina financial statements accept at most {MAX_FINANCIAL_INSTRUMENTS} instruments"
            )));
        }
        if instruments
            .iter()
            .any(|instrument| instrument.exchange() == magic_market_core::Exchange::Beijing)
        {
            return Err(SinaError::Unsupported(
                "Sina financial statements are verified only for Shanghai and Shenzhen equities"
                    .into(),
            ));
        }
        let symbols = validate_instruments(instruments)?;
        let observed_at = now()?;
        let mut records = Vec::new();
        let mut issues = Vec::new();
        for (instrument, symbol) in instruments.iter().zip(symbols) {
            let bytes = self.transport.get(&financial_url(&symbol, kind))?;
            let batch = parse_financial_response(&bytes, instrument, kind, &observed_at)?;
            issues.extend(batch.quality().issues().iter().cloned());
            records.extend(batch.into_records());
        }
        let batch_id = format!("sina-web:{observed_at}:financial:{kind:?}");
        let provenance = magic_market_core::Provenance::new("sina-web", &observed_at)?
            .with_batch_id(batch_id)?;
        Ok(DataBatch::best_effort(records, provenance, issues)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SinaClient, SinaError, SnapshotTransport};
    use magic_market_core::{
        AssetClass, Exchange, FinancialStatements, InstrumentId, ProviderId, StatementKind,
    };
    use std::sync::{Arc, Mutex};

    const INCOME_FIXTURE: &str = r#"{
      "result": {
        "status": {"code": 0},
        "data": {
          "report_count": "102",
          "report_list": {
            "20251231": {
              "rCurrency": "CNY",
              "publish_date": "20260430",
              "update_time": 1777472769,
              "data": [
                {"item_field": "", "item_title": "营业收入", "item_value": ""},
                {"item_field": "BIZINCO", "item_title": "营业收入", "item_value": "500.0"},
                {"item_field": "INTEINCO", "item_title": "利息收入", "item_value": null}
              ]
            },
            "20260331": {
              "rCurrency": "CNY",
              "publish_date": "20260430",
              "update_time": 1777472769,
              "data": [
                {"item_field": "", "item_title": "收入", "item_value": ""},
                {"item_field": "BIZINCO", "item_title": "营业收入", "item_value": "1407621057.410000"},
                {"item_field": "INTEINCO", "item_title": "利息收入", "item_value": null}
              ]
            }
          }
        }
      }
    }"#;

    #[derive(Clone)]
    struct RecordingTransport {
        response: Vec<u8>,
        urls: Arc<Mutex<Vec<String>>>,
    }

    impl SnapshotTransport for RecordingTransport {
        fn get(&self, url: &str) -> Result<Vec<u8>, SinaError> {
            self.urls.lock().unwrap().push(url.to_owned());
            Ok(self.response.clone())
        }
    }

    fn sh() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
    }

    #[test]
    fn parses_correct_report_list_shape_and_preserves_absent_values() {
        let batch = parse_financial_response(
            INCOME_FIXTURE.as_bytes(),
            &sh(),
            StatementKind::Income,
            "observed",
        )
        .unwrap();
        assert_eq!(batch.records().len(), 2);
        let latest = &batch.records()[0];
        assert_eq!(latest.report_period.as_str(), "2026-03-31");
        assert_eq!(
            latest.announced_on.as_ref().map(|date| date.as_str()),
            Some("2026-04-30")
        );
        assert_eq!(
            latest.currency.as_ref().map(|value| value.as_str()),
            Some("CNY")
        );
        assert_eq!(latest.lines.len(), 2);
        assert_eq!(latest.lines[0].key.as_str(), "bizinco");
        assert_eq!(latest.lines[0].source_label.as_str(), "营业收入");
        assert_eq!(
            latest.lines[0].value.map(|number| number.get()),
            Some(1_407_621_057.41)
        );
        assert!(latest.lines[1].value.is_none());
        assert!(latest.lines.iter().all(|line| line.unit.is_none()));
        assert_eq!(latest.evidence.provider(), ProviderId::Sina);
        assert_eq!(latest.evidence.source_at(), Some("2026-04-30"));
        assert!(batch.quality().is_complete());
    }

    #[test]
    fn statement_provider_maps_kind_and_exchange_without_guessing() {
        let urls = Arc::new(Mutex::new(Vec::new()));
        let client = SinaClient::with_transport(RecordingTransport {
            response: INCOME_FIXTURE.as_bytes().to_vec(),
            urls: Arc::clone(&urls),
        });
        let batch = client
            .financial_statements(&[sh()], StatementKind::Income)
            .unwrap();
        assert_eq!(batch.records().len(), 2);
        let urls = urls.lock().unwrap();
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("paperCode=sh600396"));
        assert!(urls[0].contains("source=lrb"));
        assert!(urls[0].contains("num=8"));
    }

    #[test]
    fn rejects_failed_status_duplicate_keys_and_non_finite_values() {
        let failed = INCOME_FIXTURE.replace("\"code\": 0", "\"code\": 12");
        assert!(matches!(
            parse_financial_response(failed.as_bytes(), &sh(), StatementKind::Income, "observed"),
            Err(SinaError::Protocol(_))
        ));
        let duplicate = INCOME_FIXTURE.replace(
            r#"{"item_field": "INTEINCO", "item_title": "利息收入", "item_value": null}"#,
            r#"{"item_field": "BIZINCO", "item_title": "重复", "item_value": "1"}"#,
        );
        assert!(matches!(
            parse_financial_response(
                duplicate.as_bytes(),
                &sh(),
                StatementKind::Income,
                "observed"
            ),
            Err(SinaError::Protocol(_))
        ));
        let non_finite = INCOME_FIXTURE.replace("1407621057.410000", "NaN");
        assert!(matches!(
            parse_financial_response(
                non_finite.as_bytes(),
                &sh(),
                StatementKind::Income,
                "observed"
            ),
            Err(SinaError::Protocol(_))
        ));
    }
}
