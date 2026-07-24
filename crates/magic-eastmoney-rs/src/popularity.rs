use crate::mapping::{
    finite, non_empty, optional_f64, optional_string, optional_u32, percent, price, required_string,
};
use crate::{
    instrument_from_market, query_url, source_instrument, BatchContext, EastmoneyClient,
    EastmoneyError,
};
use magic_market_core::{Exchange, InstrumentId, PopularityData, PopularityRank, PositiveU32};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

const RANK_ENDPOINT: &str = "https://emappdata.eastmoney.com/stockrank/getAllCurrentList";
const QUOTE_ENDPOINT: &str = "https://push2.eastmoney.com/api/qt/ulist.np/get";

impl PopularityData for EastmoneyClient {
    type Error = EastmoneyError;

    fn popularity(
        &self,
        limit: PositiveU32,
    ) -> Result<magic_market_core::DataBatch<PopularityRank>, Self::Error> {
        if limit.get() > 100 {
            return Err(EastmoneyError::InvalidRequest(
                "Eastmoney popularity limit must be at most 100".into(),
            ));
        }
        let body = serde_json::to_vec(&json!({
            "appId": "appId01",
            "globalId": "786e4c21-70dc-435a-93bb-38",
            "marketType": "",
            "pageNo": 1,
            "pageSize": limit.get()
        }))
        .map_err(|error| EastmoneyError::InvalidRequest(error.to_string()))?;
        let rank_bytes = self.post_json(
            RANK_ENDPOINT,
            &[
                ("Accept", "application/json"),
                ("Referer", "https://guba.eastmoney.com/"),
            ],
            &body,
        )?;
        let rankings = parse_rankings(&rank_bytes)?;
        if rankings.len() > limit.get() as usize {
            return Err(EastmoneyError::Protocol(format!(
                "popularity returned {} rankings for limit {}",
                rankings.len(),
                limit.get()
            )));
        }
        let secids = rankings
            .iter()
            .map(|ranking| ranking.secid.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let quotes = if secids.is_empty() {
            HashMap::new()
        } else {
            let url = query_url(
                QUOTE_ENDPOINT,
                &[
                    ("ut", "f057cbcbce2a86e2866ab8877db1d059".into()),
                    ("fltt", "2".into()),
                    ("invt", "2".into()),
                    ("fields", "f14,f3,f12,f13,f2".into()),
                    ("secids", secids),
                ],
            );
            let quote_bytes = self.get(
                &url,
                &[
                    ("Accept", "application/json"),
                    ("Referer", "https://quote.eastmoney.com/"),
                ],
            )?;
            parse_quotes(&quote_bytes)?
        };
        join_popularity(rankings, &quotes)
    }
}

struct RankingWire {
    instrument: InstrumentId,
    secid: String,
    rank: PositiveU32,
    rank_change: Option<f64>,
}

fn parse_rankings(bytes: &[u8]) -> Result<Vec<RankingWire>, EastmoneyError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    if root.get("code").and_then(Value::as_i64) != Some(0) {
        return Err(EastmoneyError::Protocol(format!(
            "popularity endpoint returned code={}",
            root.get("code").unwrap_or(&Value::Null)
        )));
    }
    let rows = root
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| EastmoneyError::Protocol("popularity data is not an array".into()))?;
    let mut seen_instruments = HashSet::with_capacity(rows.len());
    let mut seen_ranks = HashSet::with_capacity(rows.len());
    let mut rankings = Vec::with_capacity(rows.len());
    for row in rows {
        let symbol = required_string(row, "sc")?;
        let (exchange, code, market) = if let Some(code) = symbol.strip_prefix("SH") {
            (Exchange::Shanghai, code, "1")
        } else if let Some(code) = symbol.strip_prefix("SZ") {
            (Exchange::Shenzhen, code, "0")
        } else if let Some(code) = symbol.strip_prefix("BJ") {
            (Exchange::Beijing, code, "0")
        } else {
            return Err(EastmoneyError::Protocol(format!(
                "unsupported popularity symbol {symbol}"
            )));
        };
        let rank = optional_u32(row.get("rk"))?
            .ok_or_else(|| EastmoneyError::Protocol("popularity rank rk is absent".into()))?;
        let instrument = source_instrument(code, exchange)?;
        if !seen_instruments.insert(instrument.clone()) {
            return Err(EastmoneyError::Protocol(format!(
                "popularity contains duplicate instrument {symbol}"
            )));
        }
        let rank = PositiveU32::new(rank)?;
        if !seen_ranks.insert(rank) {
            return Err(EastmoneyError::Protocol(format!(
                "popularity contains duplicate rank {}",
                rank.get()
            )));
        }
        rankings.push(RankingWire {
            instrument,
            secid: format!("{market}.{code}"),
            rank,
            rank_change: optional_f64(row.get("rc"))?,
        });
    }
    Ok(rankings)
}

#[derive(Default)]
struct QuoteWire {
    name: Option<String>,
    price: Option<f64>,
    return_ratio: Option<f64>,
}

fn parse_quotes(bytes: &[u8]) -> Result<HashMap<String, QuoteWire>, EastmoneyError> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| EastmoneyError::Decode(error.to_string()))?;
    if root.get("rc").and_then(Value::as_i64) != Some(0) {
        return Err(EastmoneyError::Protocol(format!(
            "popularity quote endpoint returned rc={}",
            root.get("rc").unwrap_or(&Value::Null)
        )));
    }
    let diff = root
        .pointer("/data/diff")
        .ok_or_else(|| EastmoneyError::Protocol("popularity quote diff is absent".into()))?;
    let rows: Vec<&Value> = match diff {
        Value::Array(rows) => rows.iter().collect(),
        Value::Object(rows) => rows.values().collect(),
        Value::Null => Vec::new(),
        _ => {
            return Err(EastmoneyError::Protocol(
                "popularity quote diff is not an array or object".into(),
            ))
        }
    };
    let mut quotes = HashMap::with_capacity(rows.len());
    let mut seen_codes = HashSet::with_capacity(rows.len());
    for row in rows {
        let code = required_string(row, "f12")?;
        if !seen_codes.insert(code.clone()) {
            return Err(EastmoneyError::Protocol(format!(
                "popularity quotes contain duplicate code {code}"
            )));
        }
        let market = optional_f64(row.get("f13"))?.ok_or_else(|| {
            EastmoneyError::Protocol("popularity quote market f13 is absent".into())
        })?;
        if market.fract() != 0.0 {
            return Err(EastmoneyError::Protocol(
                "popularity quote market f13 is not integral".into(),
            ));
        }
        let instrument = instrument_from_market(&code, market as i64)?;
        let secid = format!("{}.{}", market as i64, instrument.code());
        let replaced = quotes.insert(
            secid,
            QuoteWire {
                name: optional_string(row.get("f14"))?,
                price: optional_f64(row.get("f2"))?,
                return_ratio: optional_f64(row.get("f3"))?,
            },
        );
        if replaced.is_some() {
            return Err(EastmoneyError::Protocol(format!(
                "popularity quotes contain duplicate instrument {code}"
            )));
        }
    }
    Ok(quotes)
}

fn join_popularity(
    rankings: Vec<RankingWire>,
    quotes: &HashMap<String, QuoteWire>,
) -> Result<magic_market_core::DataBatch<PopularityRank>, EastmoneyError> {
    let rank_context = BatchContext::new("popularity", None)?;
    let quote_context = BatchContext::new("popularity-quotes", None)?;
    let ranking_secids = rankings
        .iter()
        .map(|ranking| ranking.secid.as_str())
        .collect::<HashSet<_>>();
    if let Some(unexpected) = quotes
        .keys()
        .find(|secid| !ranking_secids.contains(secid.as_str()))
    {
        return Err(EastmoneyError::Protocol(format!(
            "popularity quote {unexpected} was not requested by the ranking response"
        )));
    }
    let records = rankings
        .into_iter()
        .map(|ranking| {
            let quote = quotes.get(&ranking.secid);
            Ok(PopularityRank {
                instrument: ranking.instrument,
                rank: ranking.rank,
                price: price(quote.and_then(|value| value.price))?,
                name: non_empty(quote.and_then(|value| value.name.clone()))?,
                rank_change: finite(ranking.rank_change)?,
                return_ratio: percent(quote.and_then(|value| value.return_ratio))?,
                heat: None,
                concepts: Vec::new(),
                tag: None,
                quote_evidence: quote.map(|_| quote_context.evidence()).transpose()?,
                evidence: rank_context.evidence()?,
            })
        })
        .collect::<Result<Vec<_>, EastmoneyError>>()?;
    rank_context.finish(records)
}

#[cfg(test)]
#[path = "../tests/internal/popularity_tests.rs"]
mod tests;
