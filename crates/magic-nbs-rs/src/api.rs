use crate::{max_response_bytes, transport, NbsError};
use magic_market_core::{
    DataBatch, EconomicObservation, EconomicObservationStatus, EconomicPeriod,
    EconomicSeriesRequest, FiniteNumber, Provenance, ProviderId, SourceEvidence,
};
use magic_market_transport::{HttpMethod, HttpTransport, RequestGate};
use serde::Deserialize;
use serde_json::json;

const MONTHLY: &str = "月度数据";
const PRICE_INDEX: &str = "价格指数";
const CPI_YOY_GROUP: &str = "居民消费价格分类指数 (上年同月=100)";
const CPI_2026: &str = "全国居民消费价格分类指数 (上年同月=100) (2026-)";
const PROVINCIAL_MONTHLY: &str = "分省月度数据";
const PROVINCIAL_CPI_GROUP: &str = "居民消费价格分类指数";
const PROVINCIAL_CPI_2026: &str = "居民消费价格分类指数 (上年同月=100) (2026-)";
const CPI_HEADLINE: &str = "居民消费价格指数 (上年同月=100)";
const NATIONAL_CODE: &str = "000000000000";
const BEIJING_CODE: &str = "110000000000";
const BEIJING_NAME: &str = "北京市";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdmittedScope {
    National,
    Beijing,
}

#[derive(Deserialize)]
struct ApiEnvelope<T> {
    data: T,
    success: bool,
    state: u32,
}

#[derive(Deserialize)]
struct CatalogNode {
    #[serde(rename = "_id")]
    id: String,
    name: String,
    treeinfo_pid: String,
    treeinfo_level: u32,
    #[serde(rename = "isLeaf")]
    is_leaf: bool,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct IndicatorEnvelopeData {
    total: usize,
    list: Vec<Indicator>,
}

#[derive(Deserialize)]
struct Indicator {
    #[serde(rename = "_id")]
    id: String,
    i_showname: String,
    du_name: String,
    catalogid: String,
}

#[derive(Deserialize)]
struct SeriesResult {
    code: String,
    name: String,
    values: Vec<SeriesValue>,
}

#[derive(Deserialize)]
struct SeriesValue {
    #[serde(rename = "_id")]
    id: String,
    i_showname: String,
    du_name: String,
    catalogid: String,
    value: String,
    da: String,
    da_name: String,
}

#[derive(Deserialize)]
struct AreaCatalog {
    #[serde(rename = "_id")]
    id: String,
    name: String,
    treeinfo_pid: String,
    treeinfo_level: u32,
}

#[derive(Deserialize)]
struct Area {
    catalog_id: String,
    name_value: String,
    show_name: String,
    name_text: String,
}

pub(crate) fn fetch_cpi(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    request: &EconomicSeriesRequest,
    observed_at: &str,
) -> Result<DataBatch<EconomicObservation>, NbsError> {
    let scope = validate_admitted_request(request)?;
    let (code, monthly_name, group_name, catalog_name, levels) = match scope {
        AdmittedScope::National => (1, MONTHLY, CPI_YOY_GROUP, CPI_2026, (2, 3, 4, 5)),
        AdmittedScope::Beijing => (
            4,
            PROVINCIAL_MONTHLY,
            PROVINCIAL_CPI_GROUP,
            PROVINCIAL_CPI_2026,
            (3, 4, 5, 6),
        ),
    };
    let monthly = find_catalog(transport, gate, "", code, monthly_name, levels.0, false)?;
    let price = find_catalog(
        transport,
        gate,
        &monthly,
        code,
        PRICE_INDEX,
        levels.1,
        false,
    )?;
    let group = find_catalog(transport, gate, &price, code, group_name, levels.2, false)?;
    let catalog = find_catalog(transport, gate, &group, code, catalog_name, levels.3, true)?;
    let indicator = fetch_indicator(transport, gate, &catalog)?;
    let (area_code, area_name) = match scope {
        AdmittedScope::National => (NATIONAL_CODE, "全国"),
        AdmittedScope::Beijing => {
            validate_beijing_area(transport, gate, &catalog)?;
            (BEIJING_CODE, BEIJING_NAME)
        }
    };
    let response = fetch_value(
        transport,
        gate,
        &monthly,
        &catalog,
        &indicator.id,
        area_code,
        area_name,
    )?;
    normalize_response(request, observed_at, &catalog, &indicator, response, scope)
}

fn find_catalog(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    parent: &str,
    code: u8,
    expected_name: &str,
    expected_level: u32,
    expected_leaf: bool,
) -> Result<String, NbsError> {
    let url = format!(
        "{}/new/queryIndexTreeAsync?pid={parent}&code={code}",
        transport::API_BASE
    );
    let response = transport::execute(transport, gate, HttpMethod::Get, url, Vec::new())?;
    let envelope: ApiEnvelope<Vec<CatalogNode>> = parse_json(response.body())?;
    validate_envelope(&envelope)?;
    if envelope.data.len() > 128 {
        return Err(NbsError::Protocol(
            "NBS catalog response exceeds 128 nodes".into(),
        ));
    }
    let mut matches = envelope.data.into_iter().filter(|node| {
        node.name.trim() == expected_name
            && ((parent.is_empty() && valid_id(&node.treeinfo_pid))
                || (!parent.is_empty() && node.treeinfo_pid == parent))
            && node.treeinfo_level == expected_level
            && node.is_leaf == expected_leaf
            && node.kind == "catalog"
            && valid_id(&node.id)
    });
    let matched = matches
        .next()
        .ok_or_else(|| NbsError::Protocol(format!("NBS catalog node {expected_name:?} missing")))?;
    if matches.next().is_some() {
        return Err(NbsError::Protocol(format!(
            "NBS catalog node {expected_name:?} is ambiguous"
        )));
    }
    Ok(matched.id)
}

fn fetch_indicator(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    catalog: &str,
) -> Result<Indicator, NbsError> {
    let url = format!(
        "{}/new/queryIndicatorsByCid?cid={catalog}&dt=&name=",
        transport::API_BASE
    );
    let response = transport::execute(transport, gate, HttpMethod::Get, url, Vec::new())?;
    let envelope: ApiEnvelope<IndicatorEnvelopeData> = parse_json(response.body())?;
    validate_envelope(&envelope)?;
    if envelope.data.total != envelope.data.list.len() || envelope.data.total > 64 {
        return Err(NbsError::Protocol(
            "NBS indicator catalog count is inconsistent or oversized".into(),
        ));
    }
    let mut matches = envelope.data.list.into_iter().filter(|indicator| {
        indicator.i_showname.trim() == CPI_HEADLINE
            && indicator.du_name == "%"
            && indicator.catalogid == catalog
            && valid_id(&indicator.id)
    });
    let matched = matches
        .next()
        .ok_or_else(|| NbsError::Protocol("NBS headline CPI indicator missing".into()))?;
    if matches.next().is_some() {
        return Err(NbsError::Protocol(
            "NBS headline CPI indicator is ambiguous".into(),
        ));
    }
    Ok(matched)
}

fn validate_beijing_area(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    catalog: &str,
) -> Result<(), NbsError> {
    let catalog_url = format!(
        "{}/getDaCatalogTreeByIndicatorCid?indicatorCid={catalog}",
        transport::API_BASE
    );
    let response = transport::execute(transport, gate, HttpMethod::Get, catalog_url, Vec::new())?;
    let envelope: ApiEnvelope<Vec<AreaCatalog>> = parse_json(response.body())?;
    validate_envelope(&envelope)?;
    if envelope.data.len() > 32 {
        return Err(NbsError::Protocol(
            "NBS area catalog response exceeds 32 nodes".into(),
        ));
    }
    let mut matches = envelope.data.into_iter().filter(|area| {
        area.name == "全部地区"
            && area.treeinfo_level == 2
            && valid_id(&area.id)
            && valid_id(&area.treeinfo_pid)
    });
    let area_catalog = matches
        .next()
        .ok_or_else(|| NbsError::Protocol("NBS all-regions catalog missing".into()))?;
    if matches.next().is_some() {
        return Err(NbsError::Protocol(
            "NBS all-regions catalog is ambiguous".into(),
        ));
    }

    let areas_url = format!(
        "{}/getDasByDaCatalogId?daCid={}",
        transport::API_BASE,
        area_catalog.id
    );
    let response = transport::execute(transport, gate, HttpMethod::Get, areas_url, Vec::new())?;
    let envelope: ApiEnvelope<Vec<Area>> = parse_json(response.body())?;
    validate_envelope(&envelope)?;
    if envelope.data.len() > 64 {
        return Err(NbsError::Protocol(
            "NBS area response exceeds 64 rows".into(),
        ));
    }
    let mut matches = envelope.data.into_iter().filter(|area| {
        area.catalog_id == area_catalog.id
            && area.name_value == BEIJING_CODE
            && area.show_name == BEIJING_NAME
            && area.name_text == BEIJING_NAME
    });
    matches
        .next()
        .ok_or_else(|| NbsError::Protocol("NBS Beijing area identity missing".into()))?;
    if matches.next().is_some() {
        return Err(NbsError::Protocol(
            "NBS Beijing area identity is ambiguous".into(),
        ));
    }
    Ok(())
}

fn fetch_value(
    transport: &dyn HttpTransport,
    gate: &RequestGate,
    root: &str,
    catalog: &str,
    indicator: &str,
    area_code: &str,
    area_name: &str,
) -> Result<Vec<SeriesResult>, NbsError> {
    let body = serde_json::to_vec(&json!({
        "cid": catalog,
        "indicatorIds": [indicator],
        "daCatalogId": "",
        "das": [{"text": area_name, "value": area_code}],
        "showType": "1",
        "dts": ["202607MM"],
        "rootId": root,
    }))
    .map_err(|error| NbsError::Decode(error.to_string()))?;
    let url = format!("{}/stream/esData", transport::API_BASE);
    let response = transport::execute(transport, gate, HttpMethod::Post, url, body)?;
    let envelope: ApiEnvelope<Vec<SeriesResult>> = parse_json(response.body())?;
    validate_envelope(&envelope)?;
    Ok(envelope.data)
}

fn normalize_response(
    request: &EconomicSeriesRequest,
    observed_at: &str,
    catalog: &str,
    indicator: &Indicator,
    mut results: Vec<SeriesResult>,
    scope: AdmittedScope,
) -> Result<DataBatch<EconomicObservation>, NbsError> {
    if results.len() != 1 {
        return Err(NbsError::Protocol(
            "NBS CPI response must contain exactly one period".into(),
        ));
    }
    let result = results.remove(0);
    if result.code != "202607MM" || result.name.trim() != "2026年7月" || result.values.len() != 1
    {
        return Err(NbsError::Protocol("NBS CPI period identity drifted".into()));
    }
    let row = &result.values[0];
    let (area_code, area_name, batch_prefix, provenance_source) = match scope {
        AdmittedScope::National => (
            NATIONAL_CODE,
            "全国",
            "nbs-national-cpi-yoy",
            "NBS national public-release API",
        ),
        AdmittedScope::Beijing => (
            BEIJING_CODE,
            BEIJING_NAME,
            "nbs-beijing-cpi-yoy",
            "NBS provincial public-release API",
        ),
    };
    if row.id != indicator.id
        || row.catalogid != catalog
        || row.i_showname.trim() != CPI_HEADLINE
        || row.du_name != "%"
        || row.da != area_code
        || (scope == AdmittedScope::National && !matches!(row.da_name.as_str(), "全国" | "国家"))
        || (scope == AdmittedScope::Beijing && row.da_name != area_name)
    {
        return Err(NbsError::Protocol("NBS CPI row identity drifted".into()));
    }
    let value = row
        .value
        .parse::<f64>()
        .map_err(|_| NbsError::Protocol("NBS CPI value is not numeric".into()))?;
    let batch_id = format!("{batch_prefix}:202607:{observed_at}");
    let evidence = SourceEvidence::new(ProviderId::Nbs, observed_at, &batch_id)?;
    let observation = EconomicObservation::new(
        request.series()[0].clone(),
        CPI_HEADLINE,
        None,
        None,
        EconomicPeriod::month(2026, 7)?,
        Some(FiniteNumber::new(value)?),
        "%",
        None,
        None,
        EconomicObservationStatus::Present,
        None,
        None,
        evidence,
    )?;
    let provenance = Provenance::new(provenance_source, observed_at)?.with_batch_id(batch_id)?;
    Ok(DataBatch::strict(vec![observation], provenance))
}

fn exact_common_scope(request: &EconomicSeriesRequest) -> bool {
    request.provider() == ProviderId::Nbs
        && request.series().len() == 1
        && request.series()[0].code() == "headline"
        && request.start().as_month() == Some((2026, 7))
        && request.end().as_month() == Some((2026, 7))
        && request.max_rows().get() == 1
}

pub(crate) fn validate_admitted_request(
    request: &EconomicSeriesRequest,
) -> Result<AdmittedScope, NbsError> {
    if exact_common_scope(request) {
        return match request.series()[0].namespace() {
            "national-cpi-yoy" => Ok(AdmittedScope::National),
            "beijing-cpi-yoy" => Ok(AdmittedScope::Beijing),
            _ => Err(NbsError::Unsupported(
                "only national-cpi-yoy/headline or beijing-cpi-yoy/headline for 2026-07 with max_rows=1 is admitted".into(),
            )),
        };
    }
    Err(NbsError::Unsupported(
        "only national-cpi-yoy/headline or beijing-cpi-yoy/headline for 2026-07 with max_rows=1 is admitted".into(),
    ))
}

fn parse_json<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, NbsError> {
    if body.len() > max_response_bytes() {
        return Err(NbsError::Protocol("NBS API response exceeds 4 MiB".into()));
    }
    serde_json::from_slice(body).map_err(|error| NbsError::Decode(error.to_string()))
}

fn validate_envelope<T>(envelope: &ApiEnvelope<T>) -> Result<(), NbsError> {
    if !envelope.success || envelope.state != 20000 {
        return Err(NbsError::Protocol(
            "NBS API envelope reports failure".into(),
        ));
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use magic_market_core::{EconomicSeriesKey, PositiveU32};

    fn request() -> EconomicSeriesRequest {
        EconomicSeriesRequest::new(
            vec![EconomicSeriesKey::new(ProviderId::Nbs, "national-cpi-yoy", "headline").unwrap()],
            EconomicPeriod::month(2026, 7).unwrap(),
            EconomicPeriod::month(2026, 7).unwrap(),
            PositiveU32::new(1).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn admitted_scope_is_exact() {
        assert!(validate_admitted_request(&request()).is_ok());
        let regional = EconomicSeriesRequest::new(
            vec![EconomicSeriesKey::new(ProviderId::Nbs, "beijing-cpi-yoy", "headline").unwrap()],
            EconomicPeriod::month(2026, 7).unwrap(),
            EconomicPeriod::month(2026, 7).unwrap(),
            PositiveU32::new(1).unwrap(),
        )
        .unwrap();
        assert_eq!(
            validate_admitted_request(&regional).unwrap(),
            AdmittedScope::Beijing
        );
        let wrong = EconomicSeriesRequest::new(
            vec![EconomicSeriesKey::new(ProviderId::Nbs, "national-cpi-yoy", "headline").unwrap()],
            EconomicPeriod::month(2026, 6).unwrap(),
            EconomicPeriod::month(2026, 6).unwrap(),
            PositiveU32::new(1).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            validate_admitted_request(&wrong),
            Err(NbsError::Unsupported(_))
        ));
    }

    #[test]
    fn normalized_identity_is_strict_and_keeps_source_time_absent() {
        let indicator = Indicator {
            id: "53180dfb9c14411ba4b762307c85920c".into(),
            i_showname: format!("{CPI_HEADLINE} "),
            du_name: "%".into(),
            catalogid: "5c7452825c7c4dcba391db5ca7f335c5".into(),
        };
        let result = SeriesResult {
            code: "202607MM".into(),
            name: "2026年7月".into(),
            values: vec![SeriesValue {
                id: indicator.id.clone(),
                i_showname: indicator.i_showname.clone(),
                du_name: "%".into(),
                catalogid: indicator.catalogid.clone(),
                value: "100.5".into(),
                da: NATIONAL_CODE.into(),
                da_name: "全国".into(),
            }],
        };
        let batch = normalize_response(
            &request(),
            "2026-08-13T00:00:00Z",
            &indicator.catalogid,
            &indicator,
            vec![result],
            AdmittedScope::National,
        )
        .unwrap();
        assert_eq!(batch.records()[0].value().unwrap().get(), 100.5);
        assert_eq!(batch.records()[0].unit(), "%");
        assert!(batch.records()[0].evidence().source_at().is_none());
    }
}
