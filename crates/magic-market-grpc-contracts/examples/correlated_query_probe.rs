//! Read-only correlated business RPC receipts. Plaintext is loopback-only;
//! --tls-bundle DIR also permits the known VM ports 50051 and 50056 with
//! validated mutual TLS. Capture exit zero is not business acceptance.

use std::error::Error;
use std::net::SocketAddr;
use std::time::Duration;
use std::{env, fs};

use magic_market_grpc_contracts::{v1, CANONICAL_JSON_CONTENT_TYPE, PROTOCOL_VERSION};
use prost::Message;
use sha2::{Digest, Sha256};
use tonic::metadata::MetadataValue;
use tonic::transport::{Certificate, ClientTlsConfig, Endpoint, Identity};
use tonic::{Code, Request, Status};

const ERROR_DETAIL_KEY: &str = "magic-error-detail-bin";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let tls_bundle = take_option(&mut args, "--tls-bundle")?;
    let provider_override = take_option(&mut args, "--provider")?;
    let schema_version = take_option(&mut args, "--schema-version")?
        .map(|value| value.parse::<u32>())
        .transpose()?
        .unwrap_or(1);
    if schema_version == 0 {
        return Err("schema version must be positive".into());
    }
    let receipt_json = args.last().is_some_and(|arg| arg == "--receipt-json");
    if receipt_json {
        args.pop();
    }
    let [endpoint, token_env, operation, request_id, payload_json] = args.as_slice() else {
        return Err("usage: correlated_query_probe <endpoint> <token-env-name> <operation> <request-id> <payload-json> [--tls-bundle DIR] [--provider NAME] [--schema-version N] [--receipt-json]".into());
    };
    validate_probe_endpoint(endpoint, tls_bundle.is_some())?;
    if request_id.is_empty() || request_id.len() > 128 || request_id.chars().any(char::is_control) {
        return Err("request-id must be 1..128 printable characters".into());
    }
    let (expected_operation, schema) = match operation.as_str() {
        "money-flows" => (
            v1::Operation::MoneyFlows,
            "magic.market.money_flows.request",
        ),
        "board-flows" => (
            v1::Operation::BoardFlows,
            "magic.market.board_flows.request",
        ),
        "market-announcements" => (
            v1::Operation::MarketAnnouncements,
            "magic.market.market_announcements.request",
        ),
        "historical-bars" => (
            v1::Operation::HistoricalBars,
            "magic.market.historical_bars.request",
        ),
        "global-news" => (
            v1::Operation::GlobalNews,
            "magic.market.global_news.request",
        ),
        "instrument-news" => (
            v1::Operation::InstrumentNews,
            "magic.market.instrument_news.request",
        ),
        _ => return Err("unsupported read-only probe operation".into()),
    };
    let provider = provider_override.unwrap_or_else(|| {
        match expected_operation {
            v1::Operation::MarketAnnouncements => "Cninfo",
            v1::Operation::HistoricalBars => "HithinkFinance",
            v1::Operation::GlobalNews => "WallstreetCn",
            v1::Operation::InstrumentNews => "Sina",
            _ => "Eastmoney",
        }
        .to_owned()
    });
    if provider.trim().is_empty() || provider.len() > 64 || provider.chars().any(char::is_control) {
        return Err("preferred provider must be 1..64 printable characters".into());
    }
    if payload_json.len() > 65_536
        || !serde_json::from_str::<serde_json::Value>(payload_json)?.is_object()
    {
        return Err("payload-json must be a JSON object".into());
    }
    let token = env::var(token_env).map_err(|_| "specified token environment variable is unset")?;
    if token.is_empty() {
        return Err("specified token environment variable is empty".into());
    }
    let authorization = MetadataValue::try_from(format!("Bearer {token}"))?;
    let mut endpoint_config = Endpoint::from_shared(endpoint.to_owned())?
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(90));
    if let Some(bundle) = tls_bundle.as_deref() {
        endpoint_config = endpoint_config.tls_config(client_tls_config(bundle)?)?;
    }
    let channel = endpoint_config.connect().await?;
    let mut system = v1::system_service_client::SystemServiceClient::new(channel.clone());
    let before_id = format!("{request_id}-health-before");
    let capability_id = format!("{request_id}-capabilities");
    let after_id = format!("{request_id}-health-after");
    let mut health_request = Request::new(v1::HealthRequest {
        context: Some(v1::RequestContext {
            protocol_version: PROTOCOL_VERSION,
            request_id: before_id.clone(),
        }),
    });
    health_request
        .metadata_mut()
        .insert("authorization", authorization.clone());
    let health = system.get_health(health_request).await?.into_inner();
    if health.request_id != before_id {
        return Err("health request-id mismatch".into());
    }
    let identity = health
        .build_identity
        .as_ref()
        .ok_or("health build identity missing")?;

    let capabilities = if receipt_json {
        let mut request = Request::new(v1::CapabilitiesRequest {
            context: Some(v1::RequestContext {
                protocol_version: PROTOCOL_VERSION,
                request_id: capability_id.clone(),
            }),
        });
        request
            .metadata_mut()
            .insert("authorization", authorization.clone());
        let response = system.get_capabilities(request).await?.into_inner();
        if response.request_id != capability_id {
            return Err("capabilities request-id mismatch".into());
        }
        Some(response)
    } else {
        println!("contract=ExternalV1");
        println!("service_version={}", identity.service_version);
        println!("source_revision={}", identity.source_revision);
        println!("contract_sha256={}", identity.contract_sha256);
        println!("binary_sha256={}", identity.binary_sha256);
        println!("identity_error={:?}", identity.identity_error);
        None
    };

    let payload_sha256 = format!("{:x}", Sha256::digest(payload_json.as_bytes()));
    if !receipt_json {
        println!("request_id={request_id:?}");
        println!("operation={operation}");
        println!("payload_sha256={payload_sha256}");
    }
    let mut query = Request::new(v1::QueryRequest {
        context: Some(v1::RequestContext {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.clone(),
        }),
        preferred_provider: provider.clone(),
        payload: Some(v1::CanonicalPayload {
            schema: schema.to_owned(),
            schema_version,
            content_type: CANONICAL_JSON_CONTENT_TYPE.to_owned(),
            data: payload_json.as_bytes().to_vec(),
        }),
        allow_unadmitted: false,
    });
    query
        .metadata_mut()
        .insert("authorization", authorization.clone());
    let mut market = v1::market_data_service_client::MarketDataServiceClient::new(channel);
    let result = match expected_operation {
        v1::Operation::MoneyFlows => market.money_flows(query).await,
        v1::Operation::BoardFlows => market.board_flows(query).await,
        v1::Operation::MarketAnnouncements => market.market_announcements(query).await,
        v1::Operation::HistoricalBars => market.historical_bars(query).await,
        v1::Operation::GlobalNews => market.global_news(query).await,
        v1::Operation::InstrumentNews => market.instrument_news(query).await,
        _ => unreachable!(),
    };
    if receipt_json {
        let rpc = query_receipt(
            result.map(tonic::Response::into_inner),
            request_id,
            expected_operation,
        )?;
        let mut request = Request::new(v1::HealthRequest {
            context: Some(v1::RequestContext {
                protocol_version: PROTOCOL_VERSION,
                request_id: after_id.clone(),
            }),
        });
        request
            .metadata_mut()
            .insert("authorization", authorization);
        let health_after = system.get_health(request).await?.into_inner();
        if health_after.request_id != after_id {
            return Err("post-query health request-id mismatch".into());
        }
        let same_process = health.build_identity == health_after.build_identity
            && health
                .observability
                .as_ref()
                .map(|o| o.process_started_at_unix_ms)
                == health_after
                    .observability
                    .as_ref()
                    .map(|o| o.process_started_at_unix_ms)
            && health
                .observability
                .as_ref()
                .is_some_and(|o| o.process_started_at_unix_ms > 0);
        let capabilities = capabilities.ok_or("capabilities capture missing")?;
        let entries = capabilities
            .capabilities
            .iter()
            .filter(|c| c.operation == expected_operation as i32 && c.provider == provider)
            .map(|c| {
                serde_json::json!({
                    "operation": c.operation,
                    "repository_admission": c.repository_admission,
                    "runtime_available": c.runtime_available,
                    "provider": c.provider,
                    "exact_scope": c.exact_scope,
                    "blocker": c.blocker,
                    "diagnostic_available": c.diagnostic_available
                })
            })
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::json!({
                "receipt_version": 1,
                "contract": "ExternalV1",
                "endpoint": endpoint,
                "request_id": request_id,
                "operation": expected_operation as i32,
                "request_schema": schema,
                "request_schema_version": schema_version,
                "preferred_provider": provider,
                "client_contract_sha256": format!("{:x}", Sha256::digest(v1::FILE_DESCRIPTOR_SET)),
                "request_payload_sha256": payload_sha256,
                "request_json": serde_json::from_str::<serde_json::Value>(payload_json)?,
                "health_before": health_receipt(&health),
                "health_after": health_receipt(&health_after),
                "health_before_protobuf_reencoded": protobuf_receipt(&health),
                "health_after_protobuf_reencoded": protobuf_receipt(&health_after),
                "capabilities_protobuf_reencoded": protobuf_receipt(&capabilities),
                "same_process": same_process,
                "capabilities_request_id": capabilities.request_id,
                "capabilities": entries,
                "rpc": rpc
            })
        );
        if !same_process {
            return Err("health identity/process changed across query".into());
        }
        return Ok(());
    }
    match result {
        Ok(response) => {
            let response = response.into_inner();
            if response.request_id != *request_id || response.operation != expected_operation as i32
            {
                return Err("response request-id/operation mismatch".into());
            }
            println!("grpc_code=Ok");
            println!("selected_provider={:?}", response.selected_provider);
            println!("batch_id={:?}", response.batch_id);
            println!("source_at={:?}", response.source_at);
            println!("observed_at={:?}", response.observed_at);
            println!(
                "admission={:?}",
                v1::AdmissionState::try_from(response.admission)
            );
            println!("complete={}", response.complete);
            println!("record_count={}", response.records.len());
        }
        Err(status) => print_error(&status, request_id, expected_operation)?,
    }
    Ok(())
}

fn health_receipt(health: &v1::HealthResponse) -> serde_json::Value {
    serde_json::json!({
        "request_id": health.request_id,
        "live": health.live,
        "ready": health.ready,
        "state": health.state,
        "process_started_at_unix_ms": health.observability.as_ref().map(|o| o.process_started_at_unix_ms),
        "build_identity": health.build_identity.as_ref().map(|i| serde_json::json!({
            "service_version": i.service_version,
            "source_revision": i.source_revision,
            "contract_sha256": i.contract_sha256,
            "binary_sha256": i.binary_sha256,
            "identity_error": i.identity_error
        }))
    })
}

// This is a lossless encoding of known decoded fields, not original HTTP/2
// wire bytes or an upstream Provider body. CanonicalRecord data stays exact.
fn protobuf_receipt(message: &impl Message) -> serde_json::Value {
    let bytes = message.encode_to_vec();
    serde_json::json!({
        "bytes_hex": bytes.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        "sha256": format!("{:x}", Sha256::digest(&bytes)),
        "byte_count": bytes.len()
    })
}

fn query_receipt(
    result: Result<v1::QueryResponse, Status>,
    request_id: &str,
    operation: v1::Operation,
) -> Result<serde_json::Value, Box<dyn Error>> {
    match result {
        Ok(response) => {
            if response.request_id != request_id || response.operation != operation as i32 {
                return Err("response request-id/operation mismatch".into());
            }
            let records = response
                .records
                .iter()
                .map(|record| {
                    let utf8 = std::str::from_utf8(&record.data)?;
                    Ok(serde_json::json!({
                        "schema": record.schema,
                        "schema_version": record.schema_version,
                        "content_type": record.content_type,
                        "data_sha256": format!("{:x}", Sha256::digest(&record.data)),
                        "data_utf8": utf8,
                        "decoded": serde_json::from_str::<serde_json::Value>(utf8)?
                    }))
                })
                .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
            Ok(serde_json::json!({
                "grpc_code": "OK",
                "query_response_protobuf_reencoded": protobuf_receipt(&response),
                "response": {
                    "request_id": response.request_id,
                    "operation": response.operation,
                    "admission": response.admission,
                    "selected_provider": response.selected_provider,
                    "batch_id": response.batch_id,
                    "complete": response.complete,
                    "observed_at": response.observed_at,
                    "source_at": response.source_at,
                    "diagnostic_blocker": response.diagnostic_blocker,
                    "record_count": records.len(),
                    "records": records
                }
            }))
        }
        Err(status) => {
            let bytes = status
                .metadata()
                .get_bin(ERROR_DETAIL_KEY)
                .ok_or("magic-error-detail-bin missing")?
                .to_bytes()?;
            let detail = correlated_error_detail(&bytes, request_id, operation)?;
            let attempts = detail.provider_attempts.iter().map(|a| serde_json::json!({
                "ordinal": a.ordinal, "provider": a.provider, "outcome": a.outcome,
                "reason_code": a.reason_code, "retryable": a.retryable, "terminal": a.terminal
            })).collect::<Vec<_>>();
            Ok(serde_json::json!({
                "grpc_code": format!("{:?}", status.code()),
                "grpc_message": status.message(),
                "error_detail_bytes_hex": bytes.iter().map(|b| format!("{b:02x}")).collect::<String>(),
                "error_detail_sha256": format!("{:x}", Sha256::digest(&bytes)),
                "error_detail": {
                    "request_id": detail.request_id, "operation": detail.operation,
                    "provider": detail.provider, "reason_code": detail.reason_code,
                    "retryable": detail.retryable, "admission": detail.admission,
                    "evidence_code": detail.evidence_code, "evidence_field": detail.evidence_field,
                    "record_index": detail.record_index, "has_record_index": detail.has_record_index,
                    "provider_attempts": attempts
                }
            }))
        }
    }
}

fn correlated_error_detail(
    bytes: &[u8],
    request_id: &str,
    operation: v1::Operation,
) -> Result<v1::ErrorDetail, Box<dyn Error>> {
    let detail = v1::ErrorDetail::decode(bytes)?;
    if detail.request_id != request_id || detail.operation != operation as i32 {
        return Err("error detail request-id/operation mismatch".into());
    }
    Ok(detail)
}

fn take_option(args: &mut Vec<String>, flag: &str) -> Result<Option<String>, Box<dyn Error>> {
    let Some(index) = args.iter().position(|arg| arg == flag) else {
        return Ok(None);
    };
    args.remove(index);
    if index >= args.len() || args[index].starts_with("--") {
        return Err(format!("{flag} requires a value").into());
    }
    Ok(Some(args.remove(index)))
}

fn validate_probe_endpoint(endpoint: &str, mutual_tls: bool) -> Result<(), Box<dyn Error>> {
    if !mutual_tls {
        return validate_loopback_endpoint(endpoint);
    }
    let address: SocketAddr = endpoint
        .strip_prefix("https://")
        .ok_or("TLS bundle requires HTTPS")?
        .parse()?;
    let known_vm =
        address.ip().to_string() == "10.211.55.3" && matches!(address.port(), 50051 | 50056);
    if address.port() == 0 || !(address.ip().is_loopback() || known_vm) {
        return Err("TLS probe permits only numeric loopback or known Windows VM endpoints".into());
    }
    Ok(())
}

fn read_bounded_pem(path: &std::path::Path) -> Result<Vec<u8>, Box<dyn Error>> {
    let metadata = fs::metadata(path).map_err(|_| "TLS credential file is unavailable")?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 65_536 {
        return Err("TLS credential must be a nonempty file of at most 65536 bytes".into());
    }
    let bytes = fs::read(path).map_err(|_| "TLS credential file cannot be read")?;
    if bytes.is_empty() || bytes.len() > 65_536 {
        return Err("TLS credential changed outside its read bound".into());
    }
    Ok(bytes)
}

fn client_tls_config(bundle: &str) -> Result<ClientTlsConfig, Box<dyn Error>> {
    let directory = std::path::Path::new(bundle);
    Ok(ClientTlsConfig::new()
        .domain_name("magic-market.local")
        .ca_certificate(Certificate::from_pem(read_bounded_pem(
            &directory.join("ca.pem"),
        )?))
        .identity(Identity::from_pem(
            read_bounded_pem(&directory.join("client.pem"))?,
            read_bounded_pem(&directory.join("client-key.pem"))?,
        )))
}

fn validate_loopback_endpoint(endpoint: &str) -> Result<(), Box<dyn Error>> {
    let authority = endpoint
        .strip_prefix("http://")
        .ok_or("probe requires a plaintext loopback endpoint")?;
    let address: SocketAddr = authority.parse()?;
    if !address.ip().is_loopback() {
        return Err("probe endpoint must be a numeric loopback address".into());
    }
    Ok(())
}

fn print_error(
    status: &Status,
    request_id: &str,
    operation: v1::Operation,
) -> Result<(), Box<dyn Error>> {
    println!("grpc_code={:?}", status.code());
    println!("grpc_message={:?}", status.message());
    let bytes = status
        .metadata()
        .get_bin(ERROR_DETAIL_KEY)
        .ok_or("magic-error-detail-bin missing")?
        .to_bytes()?;
    let detail = correlated_error_detail(&bytes, request_id, operation)?;
    println!("detail_request_id={:?}", detail.request_id);
    println!("detail_provider={:?}", detail.provider);
    println!("detail_reason_code={:?}", detail.reason_code);
    println!("detail_retryable={}", detail.retryable);
    println!("detail_attempt_count={}", detail.provider_attempts.len());
    for attempt in detail.provider_attempts {
        println!(
            "attempt={} provider={:?} outcome={:?} reason_code={:?} retryable={} terminal={}",
            attempt.ordinal,
            attempt.provider,
            attempt.outcome,
            attempt.reason_code,
            attempt.retryable,
            attempt.terminal
        );
    }
    if status.code() == Code::Ok {
        return Err("gRPC error status unexpectedly has OK code".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::metadata::MetadataMap;

    #[test]
    fn error_detail_requires_exact_request_correlation() {
        let detail = v1::ErrorDetail {
            request_id: "probe-1".to_owned(),
            operation: v1::Operation::MoneyFlows as i32,
            provider: "Eastmoney".to_owned(),
            reason_code: "capability_unadmitted".to_owned(),
            retryable: false,
            admission: v1::AdmissionState::Unadmitted as i32,
            evidence_code: String::new(),
            evidence_field: String::new(),
            record_index: 0,
            has_record_index: false,
            provider_attempts: Vec::new(),
        };
        let mut metadata = MetadataMap::new();
        metadata.insert_bin(
            ERROR_DETAIL_KEY,
            MetadataValue::from_bytes(&detail.encode_to_vec()),
        );
        let status = Status::with_metadata(Code::Unimplemented, "unsupported", metadata);
        assert!(print_error(&status, "probe-1", v1::Operation::MoneyFlows).is_ok());
        assert!(print_error(&status, "other", v1::Operation::MoneyFlows).is_err());
        assert!(print_error(&status, "probe-1", v1::Operation::BoardFlows).is_err());
        let receipt =
            query_receipt(Err(status.clone()), "probe-1", v1::Operation::MoneyFlows).unwrap();
        assert_eq!(receipt["grpc_code"], "Unimplemented");
        assert_eq!(receipt["error_detail"]["request_id"], "probe-1");
        assert_eq!(receipt["error_detail"]["retryable"], false);
        assert_eq!(
            receipt["error_detail_bytes_hex"],
            detail
                .encode_to_vec()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        assert!(query_receipt(Err(status), "other", v1::Operation::MoneyFlows).is_err());
    }

    #[test]
    fn json_receipt_retains_exact_record_bytes_and_rejects_wrong_correlation() {
        let bytes = br#"{ "main_net": 123.50, "status": "Available" }"#.to_vec();
        let response = v1::QueryResponse {
            request_id: "receipt-1".to_owned(),
            operation: v1::Operation::MoneyFlows as i32,
            admission: v1::AdmissionState::Admitted as i32,
            selected_provider: "Eastmoney".to_owned(),
            batch_id: "batch-1".to_owned(),
            complete: true,
            records: vec![v1::CanonicalPayload {
                schema: "magic.market.money_flow".to_owned(),
                schema_version: 1,
                content_type: CANONICAL_JSON_CONTENT_TYPE.to_owned(),
                data: bytes.clone(),
            }],
            ..Default::default()
        };
        let receipt =
            query_receipt(Ok(response.clone()), "receipt-1", v1::Operation::MoneyFlows).unwrap();
        let record = &receipt["response"]["records"][0];
        assert_eq!(record["data_utf8"], std::str::from_utf8(&bytes).unwrap());
        assert_eq!(record["decoded"]["main_net"], 123.5);
        assert_eq!(
            record["data_sha256"],
            format!("{:x}", Sha256::digest(&bytes))
        );
        let encoded = receipt["query_response_protobuf_reencoded"]["bytes_hex"]
            .as_str()
            .unwrap();
        let encoded_bytes = (0..encoded.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&encoded[index..index + 2], 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            v1::QueryResponse::decode(encoded_bytes.as_slice()).unwrap(),
            response
        );
        assert!(query_receipt(Ok(response.clone()), "other", v1::Operation::MoneyFlows).is_err());
        assert!(query_receipt(Ok(response), "receipt-1", v1::Operation::BoardFlows).is_err());
    }

    #[test]
    fn json_receipt_rejects_invalid_record_encoding() {
        for bytes in [vec![0xff], b"not JSON".to_vec()] {
            let response = v1::QueryResponse {
                request_id: "receipt-1".to_owned(),
                operation: v1::Operation::MoneyFlows as i32,
                records: vec![v1::CanonicalPayload {
                    data: bytes,
                    ..Default::default()
                }],
                ..Default::default()
            };
            assert!(query_receipt(Ok(response), "receipt-1", v1::Operation::MoneyFlows).is_err());
        }
    }

    #[test]
    fn tls_validation_never_widens_plaintext_and_rejects_unknown_targets() {
        for endpoint in [
            "https://10.211.55.3:50051",
            "https://10.211.55.3:50056",
            "https://127.0.0.1:50056",
        ] {
            assert!(validate_probe_endpoint(endpoint, true).is_ok());
            assert!(validate_probe_endpoint(endpoint, false).is_err());
        }
        for endpoint in [
            "http://10.211.55.3:50051",
            "https://10.211.55.3:50052",
            "https://192.0.2.1:50051",
            "https://localhost:50051",
            "https://10.211.55.3:50051/path",
            "https://127.0.0.1:0",
        ] {
            assert!(
                validate_probe_endpoint(endpoint, true).is_err(),
                "{endpoint}"
            );
        }
        assert!(validate_probe_endpoint("http://127.0.0.1:50056", false).is_ok());
    }

    #[test]
    fn probe_accepts_only_numeric_loopback_authorities() {
        assert!(validate_loopback_endpoint("http://127.0.0.1:50051").is_ok());
        assert!(validate_loopback_endpoint("http://[::1]:50051").is_ok());
        for endpoint in [
            "https://127.0.0.1:50051",
            "http://192.0.2.1:50051",
            "http://localhost:50051",
            "http://127.0.0.1:50051@evil.example",
            "http://127.0.0.1:50051/path",
        ] {
            assert!(validate_loopback_endpoint(endpoint).is_err(), "{endpoint}");
        }
    }
}
