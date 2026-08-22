use super::{
    batch_provenance, instrument_to_thscode, now, pair_ref, shanghai_midnight_date, source_millis,
    validate_identity, validate_safe_text, HithinkClient, HithinkError, Success,
    BALANCE_SHEETS_PATH, CASH_FLOW_STATEMENTS_PATH, FINANCIAL_STATEMENTS_ADMITTED,
    INCOME_STATEMENTS_PATH,
};
use magic_market_core::{
    AssetClass, DataBatch, FinancialLine, FinancialStatement, FinancialStatements, FiniteNumber,
    InstrumentId, IsoDate, NonEmptyText, ProviderId, SourceEvidence, StatementKind,
};
use serde::Deserialize;
use std::collections::HashSet;

const MAX_FINANCIAL_INSTRUMENTS: usize = 8;
const MAX_FINANCIAL_PERIODS: usize = 20;

impl HithinkClient {
    /// Fetches the most recent twenty quarterly consolidated statements for each instrument.
    pub fn probe_financial_statements(
        &self,
        instruments: &[InstrumentId],
        kind: StatementKind,
    ) -> Result<DataBatch<FinancialStatement>, HithinkError> {
        if instruments.is_empty() || instruments.len() > MAX_FINANCIAL_INSTRUMENTS {
            return Err(HithinkError::InvalidRequest(format!(
                "financial statement request must contain 1..={MAX_FINANCIAL_INSTRUMENTS} instruments"
            )));
        }
        let mut identities = HashSet::with_capacity(instruments.len());
        let mut responses = Vec::with_capacity(instruments.len());
        for instrument in instruments {
            if instrument.asset_class() != AssetClass::Equity {
                return Err(HithinkError::Unsupported(
                    "Fuyao company financial statements support A-share equities only".into(),
                ));
            }
            let thscode = instrument_to_thscode(instrument)?;
            if !identities.insert(thscode.clone()) {
                return Err(HithinkError::InvalidRequest(
                    "financial statement instruments must be unique".into(),
                ));
            }
            let query = [
                ("thscode", thscode),
                ("period", "quarterly".to_owned()),
                ("limit", MAX_FINANCIAL_PERIODS.to_string()),
            ];
            let response = match kind {
                StatementKind::Income => StatementResponse::Income(
                    self.get(INCOME_STATEMENTS_PATH, query.iter().map(pair_ref))?,
                ),
                StatementKind::Balance => StatementResponse::Balance(
                    self.get(BALANCE_SHEETS_PATH, query.iter().map(pair_ref))?,
                ),
                StatementKind::CashFlow => StatementResponse::CashFlow(
                    self.get(CASH_FLOW_STATEMENTS_PATH, query.iter().map(pair_ref))?,
                ),
            };
            responses.push((instrument.clone(), response));
        }
        normalize_statements(kind, responses)
    }
}

impl FinancialStatements for HithinkClient {
    type Error = HithinkError;

    fn financial_statements(
        &self,
        instruments: &[InstrumentId],
        kind: StatementKind,
    ) -> Result<DataBatch<FinancialStatement>, Self::Error> {
        if FINANCIAL_STATEMENTS_ADMITTED {
            self.probe_financial_statements(instruments, kind)
        } else {
            Err(HithinkError::Unsupported(
                "HITHINK financial statements await production admission".into(),
            ))
        }
    }
}

enum StatementResponse {
    Income(Success<FinancialData<IncomeItem>>),
    Balance(Success<FinancialData<BalanceItem>>),
    CashFlow(Success<FinancialData<CashFlowItem>>),
}

impl StatementResponse {
    fn request_id(&self) -> &str {
        match self {
            Self::Income(value) => &value.request_id,
            Self::Balance(value) => &value.request_id,
            Self::CashFlow(value) => &value.request_id,
        }
    }

    fn timestamp(&self) -> i64 {
        match self {
            Self::Income(value) => value.data.timestamp,
            Self::Balance(value) => value.data.timestamp,
            Self::CashFlow(value) => value.data.timestamp,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FinancialData<T> {
    timestamp: i64,
    item: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IncomeItem {
    thscode: String,
    ticker: String,
    period: String,
    period_end_ms: i64,
    report_date_ms: i64,
    fiscal_year: i32,
    fiscal_period: String,
    currency: String,
    basic_eps: Option<f64>,
    operating_income: Option<f64>,
    operating_costs: Option<f64>,
    operating_expenses: Option<f64>,
    operating_profit: Option<f64>,
    profit_total: Option<f64>,
    net_profit: Option<f64>,
    parent_holder_net_profit: Option<f64>,
    income_tax_expense: Option<f64>,
    interest_expenses: Option<f64>,
    manage_fee: Option<f64>,
    sales_fee: Option<f64>,
    research_and_development_expenses: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BalanceItem {
    thscode: String,
    ticker: String,
    period: String,
    period_end_ms: i64,
    report_date_ms: i64,
    fiscal_year: i32,
    fiscal_period: String,
    currency: String,
    total_current_assets: Option<f64>,
    non_current_nets_total: Option<f64>,
    assets_total: Option<f64>,
    total_debt: Option<f64>,
    holder_equity_total: Option<f64>,
    cash: Option<f64>,
    accounts_receivable: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CashFlowItem {
    thscode: String,
    ticker: String,
    period: String,
    period_end_ms: i64,
    report_date_ms: i64,
    fiscal_year: i32,
    fiscal_period: String,
    currency: String,
    act_cash_flow_net: Option<f64>,
    invest_cash_flow_net: Option<f64>,
    financing_cash_flow_net: Option<f64>,
    cash_equivalents_net_addition: Option<f64>,
    pay_dividends_profits_interest_cash: Option<f64>,
    pay_fixed_assets_etc_cash: Option<f64>,
}

struct CommonItem {
    thscode: String,
    ticker: String,
    period: String,
    period_end_ms: i64,
    report_date_ms: i64,
    fiscal_year: i32,
    fiscal_period: String,
    currency: String,
    lines: Vec<FinancialLine>,
}

fn normalize_statements(
    kind: StatementKind,
    responses: Vec<(InstrumentId, StatementResponse)>,
) -> Result<DataBatch<FinancialStatement>, HithinkError> {
    let observed_at = now()?;
    let batch_id = financial_batch_id(&responses)?;
    let mut records = Vec::new();
    let mut keys = HashSet::new();
    let mut latest_report_timestamp = None::<i64>;
    for (instrument, response) in responses {
        let response_timestamp = response.timestamp();
        let items = response_items(response)?;
        if response_timestamp <= 0 || items.is_empty() || items.len() > MAX_FINANCIAL_PERIODS {
            return Err(HithinkError::Protocol(format!(
                "financial response must contain 1..={MAX_FINANCIAL_PERIODS} periods per instrument"
            )));
        }
        if items.iter().map(|item| item.period_end_ms).max() != Some(response_timestamp) {
            return Err(HithinkError::Protocol(
                "financial response timestamp does not identify its latest report period".into(),
            ));
        }
        let expected = instrument_to_thscode(&instrument)?;
        for item in items {
            validate_identity(&expected, instrument.code(), &item.thscode, &item.ticker)?;
            if item.period != "quarterly" {
                return Err(HithinkError::Protocol(
                    "financial response period contradicts the quarterly request".into(),
                ));
            }
            validate_safe_text("fiscal_period", &item.fiscal_period)?;
            validate_financial_currency(&item.currency)?;
            let period = shanghai_midnight_date(item.period_end_ms, "period_end_ms")?;
            let announced = shanghai_midnight_date(item.report_date_ms, "report_date_ms")?;
            if period.year() != item.fiscal_year || item.report_date_ms < item.period_end_ms {
                return Err(HithinkError::Protocol(
                    "financial report dates contradict fiscal or publication order".into(),
                ));
            }
            latest_report_timestamp = Some(
                latest_report_timestamp
                    .map_or(item.report_date_ms, |value| value.max(item.report_date_ms)),
            );
            let report_period = IsoDate::new(period.to_string())?;
            if !keys.insert((instrument.clone(), report_period.clone())) {
                return Err(HithinkError::Protocol(
                    "financial response contains duplicate report periods".into(),
                ));
            }
            let evidence = SourceEvidence::new(
                ProviderId::Tonghuashun,
                observed_at.clone(),
                batch_id.clone(),
            )?
            .with_source_at(source_millis(item.report_date_ms)?)?;
            records.push(FinancialStatement {
                instrument: instrument.clone(),
                kind,
                report_period,
                announced_on: Some(IsoDate::new(announced.to_string())?),
                currency: Some(NonEmptyText::new(item.currency)?),
                lines: item.lines,
                evidence,
            });
        }
    }
    records.sort_by(|left, right| {
        left.instrument
            .code()
            .cmp(right.instrument.code())
            .then_with(|| right.report_period.cmp(&left.report_period))
    });
    let provenance = batch_provenance(
        source_millis(latest_report_timestamp.ok_or_else(|| {
            HithinkError::Protocol("financial response has no report publication time".into())
        })?)?,
        observed_at,
        batch_id,
    )?;
    Ok(DataBatch::strict(records, provenance))
}

fn response_items(response: StatementResponse) -> Result<Vec<CommonItem>, HithinkError> {
    match response {
        StatementResponse::Income(value) => value.data.item.into_iter().map(income_item).collect(),
        StatementResponse::Balance(value) => {
            value.data.item.into_iter().map(balance_item).collect()
        }
        StatementResponse::CashFlow(value) => {
            value.data.item.into_iter().map(cash_flow_item).collect()
        }
    }
}

fn validate_financial_currency(value: &str) -> Result<(), HithinkError> {
    validate_safe_text("currency", value)?;
    if value != "CNY" {
        return Err(HithinkError::Protocol(
            "A-share financial statement currency must be CNY".into(),
        ));
    }
    Ok(())
}

fn line(
    key: &'static str,
    value: Option<f64>,
    unit: &'static str,
) -> Result<FinancialLine, HithinkError> {
    Ok(FinancialLine {
        key: NonEmptyText::new(key)?,
        source_label: NonEmptyText::new(key)?,
        value: value.map(FiniteNumber::new).transpose()?,
        unit: Some(NonEmptyText::new(unit)?),
    })
}

macro_rules! common {
    ($item:ident, $lines:expr) => {
        CommonItem {
            thscode: $item.thscode,
            ticker: $item.ticker,
            period: $item.period,
            period_end_ms: $item.period_end_ms,
            report_date_ms: $item.report_date_ms,
            fiscal_year: $item.fiscal_year,
            fiscal_period: $item.fiscal_period,
            currency: $item.currency,
            lines: $lines,
        }
    };
}

fn income_item(item: IncomeItem) -> Result<CommonItem, HithinkError> {
    let lines = vec![
        line("basic_eps", item.basic_eps, "CNY/share")?,
        line("operating_income", item.operating_income, "CNY")?,
        line("operating_costs", item.operating_costs, "CNY")?,
        line("operating_expenses", item.operating_expenses, "CNY")?,
        line("operating_profit", item.operating_profit, "CNY")?,
        line("profit_total", item.profit_total, "CNY")?,
        line("net_profit", item.net_profit, "CNY")?,
        line(
            "parent_holder_net_profit",
            item.parent_holder_net_profit,
            "CNY",
        )?,
        line("income_tax_expense", item.income_tax_expense, "CNY")?,
        line("interest_expenses", item.interest_expenses, "CNY")?,
        line("manage_fee", item.manage_fee, "CNY")?,
        line("sales_fee", item.sales_fee, "CNY")?,
        line(
            "research_and_development_expenses",
            item.research_and_development_expenses,
            "CNY",
        )?,
    ];
    Ok(common!(item, lines))
}

fn balance_item(item: BalanceItem) -> Result<CommonItem, HithinkError> {
    let lines = vec![
        line("total_current_assets", item.total_current_assets, "CNY")?,
        line("non_current_nets_total", item.non_current_nets_total, "CNY")?,
        line("assets_total", item.assets_total, "CNY")?,
        line("total_debt", item.total_debt, "CNY")?,
        line("holder_equity_total", item.holder_equity_total, "CNY")?,
        line("cash", item.cash, "CNY")?,
        line("accounts_receivable", item.accounts_receivable, "CNY")?,
    ];
    Ok(common!(item, lines))
}

fn cash_flow_item(item: CashFlowItem) -> Result<CommonItem, HithinkError> {
    let lines = vec![
        line("act_cash_flow_net", item.act_cash_flow_net, "CNY")?,
        line("invest_cash_flow_net", item.invest_cash_flow_net, "CNY")?,
        line(
            "financing_cash_flow_net",
            item.financing_cash_flow_net,
            "CNY",
        )?,
        line(
            "cash_equivalents_net_addition",
            item.cash_equivalents_net_addition,
            "CNY",
        )?,
        line(
            "pay_dividends_profits_interest_cash",
            item.pay_dividends_profits_interest_cash,
            "CNY",
        )?,
        line(
            "pay_fixed_assets_etc_cash",
            item.pay_fixed_assets_etc_cash,
            "CNY",
        )?,
    ];
    Ok(common!(item, lines))
}

fn financial_batch_id(
    responses: &[(InstrumentId, StatementResponse)],
) -> Result<String, HithinkError> {
    let mut value = String::from("hithink-financials:");
    for (index, (_, response)) in responses.iter().enumerate() {
        if index > 0 {
            value.push(',');
        }
        value.push_str(response.request_id());
    }
    validate_safe_text("financial batch_id", &value)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{success, FixtureTransport};
    use crate::{parse_date, shanghai_millis};
    use magic_market_core::{Exchange, StatementKind};
    use serde_json::json;
    use time::Time;

    fn instrument() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600519", AssetClass::Equity).unwrap()
    }

    #[test]
    fn income_statements_preserve_null_lines_and_record_report_evidence() {
        let period = shanghai_millis(parse_date("2026-03-31").unwrap(), Time::MIDNIGHT).unwrap();
        let report = shanghai_millis(parse_date("2026-04-30").unwrap(), Time::MIDNIGHT).unwrap();
        let response_timestamp = period;
        let transport = FixtureTransport::new(vec![success(
            "financial-request",
            json!({
                "timestamp": response_timestamp,
                "item": [{
                    "thscode": "600519.SH",
                    "ticker": "600519",
                    "period": "quarterly",
                    "period_end_ms": period,
                    "report_date_ms": report,
                    "fiscal_year": 2026,
                    "fiscal_period": "Q1",
                    "currency": "CNY",
                    "basic_eps": 20.5,
                    "operating_income": 100.0,
                    "operating_costs": null,
                    "operating_expenses": 10.0,
                    "operating_profit": 90.0,
                    "profit_total": 88.0,
                    "net_profit": 80.0,
                    "parent_holder_net_profit": 79.0,
                    "income_tax_expense": 8.0,
                    "interest_expenses": null,
                    "manage_fee": 2.0,
                    "sales_fee": 3.0,
                    "research_and_development_expenses": 4.0
                }]
            }),
        )]);
        let observed = transport.clone();
        let client = HithinkClient::with_transport("test_key", transport).unwrap();
        let batch = client
            .probe_financial_statements(&[instrument()], StatementKind::Income)
            .unwrap();

        assert!(batch.quality().is_complete());
        assert_eq!(batch.records().len(), 1);
        assert_eq!(
            batch.provenance().source_at(),
            Some(format!("unix-ms:{report}").as_str())
        );
        let statement = &batch.records()[0];
        assert_eq!(statement.report_period.as_str(), "2026-03-31");
        assert_eq!(
            statement.announced_on.as_ref().unwrap().as_str(),
            "2026-04-30"
        );
        assert_eq!(
            statement.evidence.source_at(),
            Some(format!("unix-ms:{report}").as_str())
        );
        assert_eq!(statement.lines.len(), 13);
        assert!(statement
            .lines
            .iter()
            .find(|line| line.key.as_str() == "operating_costs")
            .unwrap()
            .value
            .is_none());
        let urls = observed.requested_urls();
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains(INCOME_STATEMENTS_PATH));
        assert!(urls[0].contains("period=quarterly"));
        assert!(urls[0].contains("limit=20"));
    }

    #[test]
    fn financials_reject_an_identity_conflict_for_the_whole_batch() {
        let timestamp = shanghai_millis(parse_date("2026-04-30").unwrap(), Time::MIDNIGHT).unwrap();
        let client = HithinkClient::with_transport(
            "test_key",
            FixtureTransport::new(vec![success(
                "financial-conflict",
                json!({
                    "timestamp": timestamp,
                    "item": [{
                        "thscode": "000001.SZ",
                        "ticker": "000001",
                        "period": "quarterly",
                        "period_end_ms": timestamp,
                        "report_date_ms": timestamp,
                        "fiscal_year": 2026,
                        "fiscal_period": "Q1",
                        "currency": "CNY",
                        "total_current_assets": null,
                        "non_current_nets_total": null,
                        "assets_total": null,
                        "total_debt": null,
                        "holder_equity_total": null,
                        "cash": null,
                        "accounts_receivable": null
                    }]
                }),
            )]),
        )
        .unwrap();
        assert!(matches!(
            client.probe_financial_statements(&[instrument()], StatementKind::Balance),
            Err(HithinkError::Protocol(_))
        ));
    }

    #[test]
    fn financial_currency_cannot_contradict_hard_coded_line_units() {
        assert!(validate_financial_currency("CNY").is_ok());
        assert!(matches!(
            validate_financial_currency("USD"),
            Err(HithinkError::Protocol(_))
        ));
    }
}
