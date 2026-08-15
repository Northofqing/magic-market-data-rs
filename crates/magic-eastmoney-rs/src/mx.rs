use crate::{observed_at, validate_instrument, EastmoneyError, EastmoneyTransport, HttpsTransport};
use magic_market_core::{
    AssetClass, DataBatch, DataStatus, Exchange, FlowInterval, FlowScope, FundFlowPoint,
    FundFlowRequest, IsoDate, Money, NonEmptyText, Provenance, ProviderId, Quantity,
    SourceEvidence,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

const ENDPOINT: &str = "https://mkapi2.dfcfs.com/finskillshub/api/claw/query";
const MAX_QUERY_BYTES: usize = 512;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_FUND_FLOW_ROWS: u32 = 20;
const SOURCE_NAME: &str = "eastmoney-miaoxiang";

pub const MX_DAILY_FUND_FLOW_ADMITTED: bool = false;
pub const MX_OPENING_AUCTION_ADMITTED: bool = false;
pub const MX_MARKET_BREADTH_ADMITTED: bool = false;

#[derive(Clone)]
struct ApiKey(Arc<str>);

impl ApiKey {
    fn new(value: impl Into<String>) -> Result<Self, EastmoneyError> {
        let value = value.into();
        let trimmed = value.trim();
        if !(8..=512).contains(&trimmed.len())
            || !trimmed.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(EastmoneyError::Authentication(
                "Eastmoney Miaoxiang API key must be 8..=512 visible ASCII bytes".into(),
            ));
        }
        Ok(Self(Arc::from(trimmed)))
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[derive(Clone)]
pub struct EastmoneyMxClient {
    transport: Arc<dyn EastmoneyTransport>,
    api_key: ApiKey,
}

impl std::fmt::Debug for EastmoneyMxClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EastmoneyMxClient")
            .field("api_key", &self.api_key)
            .finish_non_exhaustive()
    }
}

impl EastmoneyMxClient {
    pub fn from_env_with_timeout(timeout: Duration) -> Result<Self, EastmoneyError> {
        let key = api_key_from_env()?;
        Ok(Self {
            transport: Arc::new(HttpsTransport::new(timeout)?),
            api_key: ApiKey::new(key)?,
        })
    }

    /// Reuses the public Eastmoney client's transport and its single pacing
    /// gate so authenticated diagnostics cannot create a second request lane.
    pub fn from_env_with_client(client: &crate::EastmoneyClient) -> Result<Self, EastmoneyError> {
        Self::with_client(api_key_from_env()?, client)
    }

    pub fn with_client(
        api_key: impl Into<String>,
        client: &crate::EastmoneyClient,
    ) -> Result<Self, EastmoneyError> {
        Ok(Self {
            transport: Arc::clone(&client.transport),
            api_key: ApiKey::new(api_key)?,
        })
    }

    pub fn with_transport(
        api_key: impl Into<String>,
        transport: impl EastmoneyTransport + 'static,
    ) -> Result<Self, EastmoneyError> {
        Ok(Self {
            transport: Arc::new(transport),
            api_key: ApiKey::new(api_key)?,
        })
    }

    pub fn diagnose_opening_auction(
        &self,
        instrument: &magic_market_core::InstrumentId,
        trading_date: &IsoDate,
    ) -> Result<DataBatch<DiagnosticOpeningAuction>, EastmoneyError> {
        let identity = instrument_identity(instrument)?;
        let volume = self.query(&format!(
            "查询{identity}在{trading_date}的开盘集合竞价成交量"
        ))?;
        let amount = self.query(&format!(
            "查询{identity}在{trading_date}的开盘集合竞价成交额"
        ))?;
        let volume_table = volume.single_table()?;
        let amount_table = amount.single_table()?;
        volume_table.validate_security(instrument)?;
        amount_table.validate_security(instrument)?;
        volume_table.validate_single_date(trading_date)?;
        amount_table.validate_single_date(trading_date)?;
        volume_table.validate_field("开盘集合竞价成交量", "股", "DAY")?;
        amount_table.validate_field("开盘集合竞价成交额", "元", "DAY")?;
        let matched_quantity_shares = Quantity::new(parse_nonnegative_integer(
            volume_table.scalar("开盘集合竞价成交量")?,
            "opening auction volume",
        )? as f64)?;
        let matched_amount_cny = Money::new(parse_nonnegative_integer(
            amount_table.scalar("开盘集合竞价成交额")?,
            "opening auction amount",
        )? as f64)?;
        let observed = observed_at()?;
        let batch_id = format!(
            "eastmoney-mx:opening-auction:{}:{}",
            volume.request_id, amount.request_id
        );
        let evidence = source_evidence(&observed, &batch_id, trading_date.as_str())?;
        let record = DiagnosticOpeningAuction {
            instrument: instrument.clone(),
            name: NonEmptyText::new(volume_table.entity.full_name.clone())?,
            trading_date: trading_date.clone(),
            matched_price: None,
            previous_close: None,
            change_percent: None,
            matched_quantity_shares: Some(matched_quantity_shares),
            matched_amount_cny: Some(matched_amount_cny),
            unmatched_bid_quantity_shares: None,
            unmatched_ask_quantity_shares: None,
            volume_ratio: None,
            status: DataStatus::Unavailable,
            evidence,
        };
        diagnostic_batch(
            vec![record],
            observed,
            trading_date.as_str(),
            batch_id,
            "complete opening-auction fields and provider time remain unavailable",
        )
    }

    pub fn diagnose_market_breadth(
        &self,
        source_date: &IsoDate,
    ) -> Result<DataBatch<DiagnosticMarketBreadth>, EastmoneyError> {
        let response = self.query(&format!(
            "查询{source_date}A股上涨家数、下跌家数、平盘家数、涨停家数和跌停家数"
        ))?;
        if response.tables.len() != 2 {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang breadth returned {} tables, expected exactly 2",
                response.tables.len()
            )));
        }
        for table in &response.tables {
            table.validate_all_a_share_universe()?;
            table.validate_single_date(source_date)?;
        }
        let up = response.unique_u32("上涨家数")?;
        let down = response.unique_u32("下跌家数")?;
        let flat = response.unique_u32("平盘家数")?;
        let limit_up = response.unique_u32("涨停家数")?;
        let limit_down = response.unique_u32("跌停家数")?;
        let valid = up
            .checked_add(down)
            .and_then(|value| value.checked_add(flat))
            .ok_or_else(|| EastmoneyError::Protocol("breadth count overflow".into()))?;
        if limit_up > up || limit_down > down {
            return Err(EastmoneyError::Protocol(
                "Miaoxiang limit counts contradict directional counts".into(),
            ));
        }
        let observed = observed_at()?;
        let batch_id = format!("eastmoney-mx:market-breadth:{}", response.request_id);
        let evidence = source_evidence(&observed, &batch_id, source_date.as_str())?;
        let record = DiagnosticMarketBreadth {
            universe: NonEmptyText::new("all_a_shares")?,
            source_date: source_date.clone(),
            source_session: magic_market_core::MarketSession::PostClose,
            listed_total: None,
            valid,
            up,
            down,
            flat,
            limit_up,
            limit_down,
            coverage: None,
            maximum_source_skew_millis: None,
            status: DataStatus::Unavailable,
            evidence,
        };
        diagnostic_batch(
            vec![record],
            observed,
            source_date.as_str(),
            batch_id,
            "listed total, coverage and source-time skew remain unavailable",
        )
    }

    pub fn diagnose_daily_fund_flow(
        &self,
        request: &FundFlowRequest,
    ) -> Result<DataBatch<FundFlowPoint>, EastmoneyError> {
        let FlowScope::Instrument(instrument) = request.scope() else {
            return Err(EastmoneyError::Unsupported(
                "Miaoxiang daily fund-flow diagnostic requires one instrument".into(),
            ));
        };
        if request.interval() != FlowInterval::Day1 {
            return Err(EastmoneyError::Unsupported(
                "Miaoxiang fund-flow diagnostic proves only Day1 granularity".into(),
            ));
        }
        if request.limit().get() > MAX_FUND_FLOW_ROWS {
            return Err(EastmoneyError::InvalidRequest(format!(
                "Miaoxiang fund-flow limit must be in 1..={MAX_FUND_FLOW_ROWS}"
            )));
        }
        let identity = instrument_identity(instrument)?;
        let response = self.query(&format!(
            "查询{identity}最近{}个交易日的主力资金净流入、超大单、大单、中单、小单净流入",
            request.limit().get()
        ))?;
        let table = response
            .tables
            .iter()
            .filter(|table| table.has_label("(区间)主力净流入资金"))
            .collect::<Vec<_>>();
        let [table] = table.as_slice() else {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang fund-flow returned {} matching tables, expected exactly 1",
                table.len()
            )));
        };
        table.validate_security(instrument)?;
        table.validate_field("(区间)主力净流入资金", "元", "DAY")?;
        let dates = table.dates()?;
        if dates.len() < request.limit().get() as usize
            || dates.len() > request.limit().get() as usize + 2
            || dates.len() > MAX_FUND_FLOW_ROWS as usize + 2
        {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang fund-flow returned {} dates for requested limit {}",
                dates.len(),
                request.limit().get()
            )));
        }
        validate_descending_dates(dates)?;
        let labels = [
            "(区间)主力净流入资金",
            "(区间)超大单净流入资金",
            "(区间)大单净流入资金",
            "(区间)中单净流入资金",
            "(区间)小单净流入资金",
        ];
        let columns = labels
            .iter()
            .map(|label| table.column(label, dates.len()))
            .collect::<Result<Vec<_>, _>>()?;
        let observed = observed_at()?;
        let batch_id = format!("eastmoney-mx:daily-fund-flow:{}", response.request_id);
        let selected = request.limit().get() as usize;
        let mut records = (0..selected)
            .map(|index| {
                let date = IsoDate::new(dates[index].clone())?;
                Ok(FundFlowPoint {
                    scope: FlowScope::Instrument(instrument.clone()),
                    interval: FlowInterval::Day1,
                    period_at: NonEmptyText::new(date.as_str())?,
                    main_net: Some(parse_money(columns[0][index], "main net")?),
                    main_ratio: None,
                    super_large_net: Some(parse_money(columns[1][index], "super-large net")?),
                    large_net: Some(parse_money(columns[2][index], "large net")?),
                    medium_net: Some(parse_money(columns[3][index], "medium net")?),
                    small_net: Some(parse_money(columns[4][index], "small net")?),
                    evidence: source_evidence(&observed, &batch_id, date.as_str())?,
                })
            })
            .collect::<Result<Vec<_>, EastmoneyError>>()?;
        records.reverse();
        let newest = records
            .last()
            .ok_or_else(|| EastmoneyError::Protocol("fund-flow records are empty".into()))?
            .period_at
            .as_str()
            .to_owned();
        diagnostic_batch(
            records,
            observed,
            &newest,
            batch_id,
            "natural-language result cardinality is bounded but repository-unadmitted",
        )
    }

    fn query(&self, query: &str) -> Result<MxResponse, EastmoneyError> {
        if query.is_empty() || query.len() > MAX_QUERY_BYTES || query.chars().any(char::is_control)
        {
            return Err(EastmoneyError::InvalidRequest(format!(
                "Miaoxiang query must be non-empty, control-free and at most {MAX_QUERY_BYTES} bytes"
            )));
        }
        let body = serde_json::to_vec(&QueryBody { tool_query: query })
            .map_err(|error| EastmoneyError::Decode(error.to_string()))?;
        let bytes = self.transport.post_json(
            ENDPOINT,
            &[
                ("Accept", "application/json"),
                ("apikey", self.api_key.expose()),
            ],
            &body,
            MAX_RESPONSE_BYTES,
        )?;
        MxResponse::parse(&bytes)
    }
}

fn api_key_from_env() -> Result<String, EastmoneyError> {
    std::env::var("EASTMONEY_API_KEY")
        .or_else(|_| std::env::var("MX_APIKEY"))
        .map_err(|_| {
            EastmoneyError::Authentication(
                "set EASTMONEY_API_KEY (or compatibility alias MX_APIKEY) in the server process"
                    .into(),
            )
        })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiagnosticOpeningAuction {
    pub instrument: magic_market_core::InstrumentId,
    pub name: NonEmptyText,
    pub trading_date: IsoDate,
    pub matched_price: Option<magic_market_core::Price>,
    pub previous_close: Option<magic_market_core::Price>,
    pub change_percent: Option<magic_market_core::Ratio>,
    pub matched_quantity_shares: Option<Quantity>,
    pub matched_amount_cny: Option<Money>,
    pub unmatched_bid_quantity_shares: Option<Quantity>,
    pub unmatched_ask_quantity_shares: Option<Quantity>,
    pub volume_ratio: Option<magic_market_core::Ratio>,
    pub status: DataStatus,
    pub evidence: SourceEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiagnosticMarketBreadth {
    pub universe: NonEmptyText,
    pub source_date: IsoDate,
    pub source_session: magic_market_core::MarketSession,
    pub listed_total: Option<u32>,
    pub valid: u32,
    pub up: u32,
    pub down: u32,
    pub flat: u32,
    pub limit_up: u32,
    pub limit_down: u32,
    pub coverage: Option<magic_market_core::Ratio>,
    pub maximum_source_skew_millis: Option<u64>,
    pub status: DataStatus,
    pub evidence: SourceEvidence,
}

#[derive(Serialize)]
struct QueryBody<'a> {
    #[serde(rename = "toolQuery")]
    tool_query: &'a str,
}

#[derive(Deserialize)]
struct OuterEnvelope {
    success: bool,
    status: i64,
    code: i64,
    message: String,
    data: InnerEnvelope,
    #[serde(rename = "requestId")]
    request_id: String,
}

#[derive(Deserialize)]
struct InnerEnvelope {
    status: i64,
    code: i64,
    message: String,
    data: SearchPayload,
}

#[derive(Deserialize)]
struct SearchPayload {
    #[serde(rename = "protocolType")]
    protocol_type: String,
    id: String,
    #[serde(rename = "searchDataResultDTO")]
    search: SearchResult,
}

#[derive(Deserialize)]
struct SearchResult {
    #[serde(rename = "dataTableDTOList")]
    tables: Vec<MxTable>,
}

struct MxResponse {
    request_id: String,
    tables: Vec<MxTable>,
}

impl MxResponse {
    fn parse(bytes: &[u8]) -> Result<Self, EastmoneyError> {
        let envelope: OuterEnvelope = serde_json::from_slice(bytes)
            .map_err(|error| EastmoneyError::Decode(error.to_string()))?;
        if !envelope.success || envelope.status != 0 || envelope.code != 0 {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang outer status failed: success={}, status={}, code={}, message={:?}",
                envelope.success, envelope.status, envelope.code, envelope.message
            )));
        }
        if envelope.data.status != 0 || envelope.data.code != 0 {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang inner status failed: status={}, code={}, message={:?}",
                envelope.data.status, envelope.data.code, envelope.data.message
            )));
        }
        if envelope.data.data.protocol_type != "SEARCH_DATA" {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang protocol type {:?} is not SEARCH_DATA",
                envelope.data.data.protocol_type
            )));
        }
        validate_identifier("outer requestId", &envelope.request_id)?;
        validate_identifier("payload id", &envelope.data.data.id)?;
        Ok(Self {
            request_id: envelope.request_id,
            tables: envelope.data.data.search.tables,
        })
    }

    fn single_table(&self) -> Result<&MxTable, EastmoneyError> {
        let [table] = self.tables.as_slice() else {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang returned {} tables, expected exactly 1",
                self.tables.len()
            )));
        };
        Ok(table)
    }

    fn unique_u32(&self, label: &str) -> Result<u32, EastmoneyError> {
        let matches = self
            .tables
            .iter()
            .filter(|table| table.has_label(label))
            .collect::<Vec<_>>();
        let [table] = matches.as_slice() else {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang returned {} tables for {label:?}, expected exactly 1",
                matches.len()
            )));
        };
        table
            .scalar(label)?
            .parse::<u32>()
            .map_err(|error| EastmoneyError::Protocol(format!("invalid {label} count: {error}")))
    }
}

#[derive(Deserialize)]
struct MxTable {
    code: String,
    #[serde(rename = "entityName")]
    entity_name: String,
    #[serde(rename = "rawTable")]
    raw_table: BTreeMap<String, Vec<String>>,
    #[serde(rename = "nameMap")]
    name_map: BTreeMap<String, String>,
    field: MxField,
    #[serde(rename = "entityTagDTO")]
    entity: MxEntity,
}

impl MxTable {
    fn has_label(&self, label: &str) -> bool {
        self.name_map.values().any(|value| value == label)
    }

    fn key_for_label(&self, label: &str) -> Result<&str, EastmoneyError> {
        let keys = self
            .name_map
            .iter()
            .filter_map(|(key, value)| (value == label).then_some(key.as_str()))
            .collect::<Vec<_>>();
        let [key] = keys.as_slice() else {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang table has {} keys for {label:?}, expected exactly 1",
                keys.len()
            )));
        };
        Ok(key)
    }

    fn scalar(&self, label: &str) -> Result<&str, EastmoneyError> {
        let key = self.key_for_label(label)?;
        let values = self.raw_table.get(key).ok_or_else(|| {
            EastmoneyError::Protocol(format!("Miaoxiang raw table has no column {key:?}"))
        })?;
        let [value] = values.as_slice() else {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang {label:?} has {} values, expected exactly 1",
                values.len()
            )));
        };
        Ok(value)
    }

    fn column(&self, label: &str, expected: usize) -> Result<Vec<&str>, EastmoneyError> {
        let key = self.key_for_label(label)?;
        let values = self.raw_table.get(key).ok_or_else(|| {
            EastmoneyError::Protocol(format!("Miaoxiang raw table has no column {key:?}"))
        })?;
        if values.len() != expected {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang {label:?} has {} values, expected {expected}",
                values.len()
            )));
        }
        Ok(values.iter().map(String::as_str).collect())
    }

    fn dates(&self) -> Result<&[String], EastmoneyError> {
        self.raw_table
            .get("headName")
            .map(Vec::as_slice)
            .ok_or_else(|| EastmoneyError::Protocol("Miaoxiang table has no headName dates".into()))
    }

    fn validate_single_date(&self, date: &IsoDate) -> Result<(), EastmoneyError> {
        let [actual] = self.dates()? else {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang table has {} dates, expected exactly 1",
                self.dates()?.len()
            )));
        };
        if actual != date.as_str() {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang source date {actual:?} does not match requested {date}"
            )));
        }
        Ok(())
    }

    fn validate_field(
        &self,
        label: &str,
        unit: &str,
        granularity: &str,
    ) -> Result<(), EastmoneyError> {
        let key = self.key_for_label(label)?;
        if self.field.return_code != key
            || self.field.return_name != label
            || self.field.unit_name.as_deref() != Some(unit)
            || self.field.date_granularity != granularity
        {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang field metadata does not prove {label:?}/{unit}/{granularity}"
            )));
        }
        Ok(())
    }

    fn validate_security(
        &self,
        expected: &magic_market_core::InstrumentId,
    ) -> Result<(), EastmoneyError> {
        let identity = instrument_identity(expected)?;
        let expected_suffix = exchange_suffix(expected.exchange())?;
        let expected_market = format!(".{expected_suffix}");
        if self.code != identity
            || self.entity.secu_code.as_deref() != Some(expected.code())
            || self.entity.market_char.as_deref() != Some(expected_market.as_str())
            || self.entity.entity_type_name != "A股"
            || self.entity.full_name.trim().is_empty()
            || !self.entity_name.ends_with(&format!("({identity})"))
        {
            return Err(EastmoneyError::Protocol(format!(
                "Miaoxiang security identity does not match requested {identity}"
            )));
        }
        Ok(())
    }

    fn validate_all_a_share_universe(&self) -> Result<(), EastmoneyError> {
        if self.code != "001071"
            || self.entity_name != "全部A股(板块)"
            || self.entity.entity_type != "BLOCK"
            || self.entity.full_name != "全部A股"
            || self.entity.class_name != "市场类(沪深京)"
        {
            return Err(EastmoneyError::Protocol(
                "Miaoxiang breadth universe is not the exact all-A-share aggregate".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct MxField {
    #[serde(rename = "returnCode")]
    return_code: String,
    #[serde(rename = "returnName")]
    return_name: String,
    #[serde(rename = "dateGranularity")]
    date_granularity: String,
    #[serde(rename = "unitName")]
    unit_name: Option<String>,
}

#[derive(Deserialize)]
struct MxEntity {
    #[serde(rename = "entityType")]
    entity_type: String,
    #[serde(rename = "entityTypeName")]
    entity_type_name: String,
    #[serde(rename = "className")]
    class_name: String,
    #[serde(rename = "fullName")]
    full_name: String,
    #[serde(rename = "secuCode")]
    secu_code: Option<String>,
    #[serde(rename = "marketChar")]
    market_char: Option<String>,
}

fn instrument_identity(
    instrument: &magic_market_core::InstrumentId,
) -> Result<String, EastmoneyError> {
    validate_instrument(instrument)?;
    if instrument.asset_class() != AssetClass::Equity {
        return Err(EastmoneyError::Unsupported(
            "Miaoxiang diagnostic supports only A-share equities".into(),
        ));
    }
    Ok(format!(
        "{}.{}",
        instrument.code(),
        exchange_suffix(instrument.exchange())?
    ))
}

fn exchange_suffix(exchange: Exchange) -> Result<&'static str, EastmoneyError> {
    match exchange {
        Exchange::Shanghai => Ok("SH"),
        Exchange::Shenzhen => Ok("SZ"),
        Exchange::Beijing => Err(EastmoneyError::Unsupported(
            "Miaoxiang Beijing diagnostic identity is not yet live-proved".into(),
        )),
    }
}

fn validate_identifier(field: &str, value: &str) -> Result<(), EastmoneyError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(EastmoneyError::Protocol(format!(
            "Miaoxiang {field} is invalid"
        )));
    }
    Ok(())
}

fn parse_nonnegative_integer(value: &str, field: &str) -> Result<u64, EastmoneyError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(EastmoneyError::Protocol(format!(
            "Miaoxiang {field} is not a non-negative integer"
        )));
    }
    let value = value.parse::<u64>().map_err(|error| {
        EastmoneyError::Protocol(format!("Miaoxiang {field} is invalid: {error}"))
    })?;
    if value > (1_u64 << 53) {
        return Err(EastmoneyError::Protocol(format!(
            "Miaoxiang {field} exceeds exact f64 integer range"
        )));
    }
    Ok(value)
}

fn parse_money(value: &str, field: &str) -> Result<Money, EastmoneyError> {
    let value = value.parse::<f64>().map_err(|error| {
        EastmoneyError::Protocol(format!("Miaoxiang {field} is invalid: {error}"))
    })?;
    Ok(Money::new(value)?)
}

fn validate_descending_dates(dates: &[String]) -> Result<(), EastmoneyError> {
    let parsed = dates
        .iter()
        .map(|date| IsoDate::new(date.clone()).map_err(EastmoneyError::Core))
        .collect::<Result<Vec<_>, _>>()?;
    if parsed.windows(2).any(|window| window[0] <= window[1]) {
        return Err(EastmoneyError::Protocol(
            "Miaoxiang fund-flow dates must be unique newest-first".into(),
        ));
    }
    Ok(())
}

fn source_evidence(
    observed: &str,
    batch_id: &str,
    source_at: &str,
) -> Result<SourceEvidence, EastmoneyError> {
    Ok(
        SourceEvidence::new(ProviderId::Eastmoney, observed, batch_id)?
            .with_source_at(source_at)?,
    )
}

fn diagnostic_batch<T>(
    records: Vec<T>,
    observed: String,
    source_at: &str,
    batch_id: String,
    issue: &str,
) -> Result<DataBatch<T>, EastmoneyError> {
    let provenance = Provenance::new(SOURCE_NAME, observed)?
        .with_source_at(source_at)?
        .with_batch_id(batch_id)?;
    Ok(DataBatch::best_effort(
        records,
        provenance,
        vec![issue.to_owned()],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use magic_market_core::{InstrumentId, PositiveU32};
    use serde_json::json;
    use std::sync::Mutex;

    type ObservedRequest = (String, Vec<(String, String)>, Vec<u8>);

    const _: () = assert!(!MX_DAILY_FUND_FLOW_ADMITTED);
    const _: () = assert!(!MX_OPENING_AUCTION_ADMITTED);
    const _: () = assert!(!MX_MARKET_BREADTH_ADMITTED);

    #[derive(Clone)]
    struct FixtureTransport {
        responses: Arc<Mutex<Vec<Vec<u8>>>>,
        requests: Arc<Mutex<Vec<ObservedRequest>>>,
    }

    impl FixtureTransport {
        fn new(responses: Vec<serde_json::Value>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(
                    responses
                        .into_iter()
                        .rev()
                        .map(|value| serde_json::to_vec(&value).unwrap())
                        .collect(),
                )),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl EastmoneyTransport for FixtureTransport {
        fn get(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            unreachable!()
        }

        fn post_json(
            &self,
            url: &str,
            headers: &[(&str, &str)],
            body: &[u8],
            _max_bytes: usize,
        ) -> Result<Vec<u8>, EastmoneyError> {
            self.requests.lock().unwrap().push((
                url.to_owned(),
                headers
                    .iter()
                    .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                    .collect(),
                body.to_vec(),
            ));
            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| EastmoneyError::Transport("fixture exhausted".into()))
        }
    }

    fn instrument() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
    }

    #[test]
    fn mx_client_can_share_the_public_clients_single_transport_lane() {
        let public = crate::EastmoneyClient::with_transport(FixtureTransport::new(Vec::new()));
        let mx = EastmoneyMxClient::with_client("mkt_test_key", &public).unwrap();
        assert!(Arc::ptr_eq(&public.transport, &mx.transport));
    }

    fn entity() -> serde_json::Value {
        json!({
            "entityType": "SEC",
            "entityTypeName": "A股",
            "className": "沪深京股票",
            "fullName": "华电辽能",
            "secuCode": "600396",
            "marketChar": ".SH"
        })
    }

    fn envelope(id: &str, tables: Vec<serde_json::Value>) -> serde_json::Value {
        json!({
            "success": true,
            "status": 0,
            "code": 0,
            "message": "ok",
            "requestId": id,
            "data": {
                "status": 0,
                "code": 0,
                "message": "OK",
                "data": {
                    "protocolType": "SEARCH_DATA",
                    "id": format!("payload-{id}"),
                    "searchDataResultDTO": { "dataTableDTOList": tables }
                }
            }
        })
    }

    fn scalar_table(label: &str, code: &str, value: &str, unit: Option<&str>) -> serde_json::Value {
        json!({
            "code": "600396.SH",
            "entityName": "华电辽能(600396.SH)",
            "rawTable": { code: [value], "headName": ["2026-08-14"] },
            "nameMap": { code: label, "headNameSub": "数据来源" },
            "field": {
                "returnCode": code,
                "returnName": label,
                "dateGranularity": "DAY",
                "unitName": unit
            },
            "entityTagDTO": entity()
        })
    }

    #[test]
    fn debug_redacts_key_and_admission_stays_false() {
        let client = EastmoneyMxClient::with_transport(
            "mkt_secret_value",
            FixtureTransport::new(Vec::new()),
        )
        .unwrap();
        let debug = format!("{client:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("mkt_secret_value"));
    }

    #[test]
    fn opening_auction_preserves_observed_fields_and_nulls_unproved_fields() {
        let transport = FixtureTransport::new(vec![
            envelope(
                "volume-request",
                vec![scalar_table(
                    "开盘集合竞价成交量",
                    "100000000047336",
                    "2951900",
                    Some("股"),
                )],
            ),
            envelope(
                "amount-request",
                vec![scalar_table(
                    "开盘集合竞价成交额",
                    "100000000047337",
                    "53665542",
                    Some("元"),
                )],
            ),
        ]);
        let observed = transport.clone();
        let client = EastmoneyMxClient::with_transport("mkt_test_key", transport).unwrap();
        let batch = client
            .diagnose_opening_auction(&instrument(), &IsoDate::new("2026-08-14").unwrap())
            .unwrap();
        let record = &batch.records()[0];
        assert_eq!(record.matched_quantity_shares.unwrap().get(), 2_951_900.0);
        assert_eq!(record.matched_amount_cny.unwrap().get(), 53_665_542.0);
        assert!(record.matched_price.is_none());
        assert!(record.unmatched_bid_quantity_shares.is_none());
        assert_eq!(record.status, DataStatus::Unavailable);
        assert!(!batch.quality().is_complete());
        let requests = observed.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|request| request.0 == ENDPOINT));
        assert!(requests.iter().all(|request| request
            .1
            .iter()
            .any(|(name, value)| name == "apikey" && value == "mkt_test_key")));
    }

    #[test]
    fn opening_auction_rejects_unproved_unit() {
        let client = EastmoneyMxClient::with_transport(
            "mkt_test_key",
            FixtureTransport::new(vec![
                envelope(
                    "volume-request",
                    vec![scalar_table(
                        "开盘集合竞价成交量",
                        "100000000047336",
                        "2951900",
                        Some("手"),
                    )],
                ),
                envelope(
                    "amount-request",
                    vec![scalar_table(
                        "开盘集合竞价成交额",
                        "100000000047337",
                        "53665542",
                        Some("元"),
                    )],
                ),
            ]),
        )
        .unwrap();
        assert!(matches!(
            client.diagnose_opening_auction(&instrument(), &IsoDate::new("2026-08-14").unwrap()),
            Err(EastmoneyError::Protocol(_))
        ));
    }

    fn breadth_table(fields: &[(&str, &str, &str)], return_code: &str) -> serde_json::Value {
        let raw = fields
            .iter()
            .map(|(_, code, value)| ((*code).to_owned(), json!([value])))
            .chain(std::iter::once((
                "headName".to_owned(),
                json!(["2026-08-14"]),
            )))
            .collect::<serde_json::Map<_, _>>();
        let names = fields
            .iter()
            .map(|(label, code, _)| ((*code).to_owned(), json!(label)))
            .collect::<serde_json::Map<_, _>>();
        json!({
            "code": "001071",
            "entityName": "全部A股(板块)",
            "rawTable": raw,
            "nameMap": names,
            "field": {
                "returnCode": return_code,
                "returnName": fields.iter().find(|(_, code, _)| *code == return_code).unwrap().0,
                "dateGranularity": "DAY",
                "unitName": null
            },
            "entityTagDTO": {
                "entityType": "BLOCK",
                "entityTypeName": "BLOCK",
                "className": "市场类(沪深京)",
                "fullName": "全部A股"
            }
        })
    }

    #[test]
    fn breadth_retains_counts_but_not_unproved_total_or_coverage() {
        let client = EastmoneyMxClient::with_transport(
            "mkt_test_key",
            FixtureTransport::new(vec![envelope(
                "breadth-request",
                vec![
                    breadth_table(
                        &[
                            ("上涨家数", "up", "2400"),
                            ("下跌家数", "down", "2970"),
                            ("平盘家数", "flat", "170"),
                        ],
                        "down",
                    ),
                    breadth_table(
                        &[
                            ("涨停家数", "limit-up", "64"),
                            ("跌停家数", "limit-down", "13"),
                        ],
                        "limit-down",
                    ),
                ],
            )]),
        )
        .unwrap();
        let batch = client
            .diagnose_market_breadth(&IsoDate::new("2026-08-14").unwrap())
            .unwrap();
        let record = &batch.records()[0];
        assert_eq!((record.up, record.down, record.flat), (2400, 2970, 170));
        assert_eq!((record.limit_up, record.limit_down), (64, 13));
        assert_eq!(record.valid, 5540);
        assert!(record.listed_total.is_none());
        assert!(record.coverage.is_none());
        assert_eq!(record.status, DataStatus::Unavailable);
    }

    #[test]
    fn daily_fund_flow_is_bounded_and_reordered_oldest_first() {
        let labels = [
            ("(区间)主力净流入资金", "main", ["30", "20", "10"]),
            ("(区间)超大单净流入资金", "super", ["3", "2", "1"]),
            ("(区间)大单净流入资金", "large", ["6", "4", "2"]),
            ("(区间)中单净流入资金", "medium", ["-3", "-2", "-1"]),
            ("(区间)小单净流入资金", "small", ["-6", "-4", "-2"]),
        ];
        let raw = labels
            .iter()
            .map(|(_, code, values)| ((*code).to_owned(), json!(values)))
            .chain(std::iter::once((
                "headName".to_owned(),
                json!(["2026-08-14", "2026-08-13", "2026-08-12"]),
            )))
            .collect::<serde_json::Map<_, _>>();
        let names = labels
            .iter()
            .map(|(label, code, _)| ((*code).to_owned(), json!(label)))
            .collect::<serde_json::Map<_, _>>();
        let table = json!({
            "code": "600396.SH",
            "entityName": "华电辽能(600396.SH)",
            "rawTable": raw,
            "nameMap": names,
            "field": {
                "returnCode": "main",
                "returnName": "(区间)主力净流入资金",
                "dateGranularity": "DAY",
                "unitName": "元"
            },
            "entityTagDTO": entity()
        });
        let client = EastmoneyMxClient::with_transport(
            "mkt_test_key",
            FixtureTransport::new(vec![envelope("flow-request", vec![table])]),
        )
        .unwrap();
        let request = FundFlowRequest::new(
            FlowScope::Instrument(instrument()),
            FlowInterval::Day1,
            PositiveU32::new(2).unwrap(),
        )
        .unwrap();
        let batch = client.diagnose_daily_fund_flow(&request).unwrap();
        assert_eq!(batch.records().len(), 2);
        assert_eq!(batch.records()[0].period_at.as_str(), "2026-08-13");
        assert_eq!(batch.records()[1].period_at.as_str(), "2026-08-14");
        assert_eq!(batch.records()[1].main_net.unwrap().get(), 30.0);
        assert!(!batch.quality().is_complete());
    }

    #[test]
    fn failed_outer_status_is_not_a_successful_empty_result() {
        let mut value = envelope("failed-request", Vec::new());
        value["status"] = json!(1001);
        let client =
            EastmoneyMxClient::with_transport("mkt_test_key", FixtureTransport::new(vec![value]))
                .unwrap();
        assert!(matches!(
            client.diagnose_market_breadth(&IsoDate::new("2026-08-14").unwrap()),
            Err(EastmoneyError::Protocol(_))
        ));
    }
}
