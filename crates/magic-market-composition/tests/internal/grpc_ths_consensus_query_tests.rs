use super::register_ths_consensus_client;
use magic_market_core::ConsensusSnapshot;
use magic_market_service::{
    CanonicalPayload, Operation, OperationRegistry, QueryCommand, ServiceError,
};
use magic_ths_rs::{
    HttpMethod, HttpRequest, HttpResponse, ThsClient, ThsConfig, ThsError, ThsTransport,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAXIMUM_PAYLOAD_BYTES: usize = 16 * 1024;
const COMPLETE_URL: &str = "https://basic.10jqka.com.cn/600519/worth.html";
const EMPTY_URL: &str = "https://basic.10jqka.com.cn/600396/worth.html";

// Fixed GBK bytes of the pinned, checked-in external HTML samples.
// Encoding is fixture preparation only; the real SDK performs HTML decoding/parsing.
const COMPLETE_HTML_GBK_HEX: &str = concat!(
    "3c68746d6c3e0a3c686561643e3c7469746c653eb9f3d6ddc3a9cca82836303035313929bcdbd6b5",
    "b7d6cef63c2f7469746c653e3c2f686561643e0a3c626f64793e0a3c703ebdd8d6c1323032362d30",
    "372d3233a3ac36b8f6d4c2d2d4c4dab9b2d3d020343620bcd2bbfab9b9d7f7b3f6d4a4b2e2a1a33c",
    "2f703e0a3c7461626c6520636c6173733d226d5f7461626c65206d5f686c223e0a20203c63617074",
    "696f6e3e3c7370616e3eb5a5cebba3bad4aa3c2f7370616e3ebbe3d7dc2d2dd4a4b2e2c4eab1a8c3",
    "bfb9c9cad5d2e63c2f63617074696f6e3e0a20203c74686561643e0a202020203c74723e0a202020",
    "2020203c74683ec4eab6c83c2f74683e0a2020202020203c74683ed4a4b2e2bbfab9b9cafd3c2f74",
    "683e0a2020202020203c74683ed7eed0a1d6b53c2f74683e0a2020202020203c74683ebef9d6b53c",
    "2f74683e0a2020202020203c74683ed7eeb4f3d6b53c2f74683e0a2020202020203c74683ed0d0d2",
    "b5c6bdbef9cafd3c2f74683e0a202020203c2f74723e0a20203c2f74686561643e0a20203c74626f",
    "64793e0a202020203c74723e3c74683e323032363c2f74683e3c74643e34363c2f74643e3c74643e",
    "36352e30323c2f74643e3c74643e36382e37333c2f74643e3c74643e37372e38353c2f74643e3c74",
    "643e382e35333c2f74643e3c2f74723e0a202020203c74723e3c74683e323032373c2f74683e3c74",
    "643e34363c2f74643e3c74643e36372e36313c2f74643e3c74643e37322e34383c2f74643e3c7464",
    "3e38342e30323c2f74643e3c74643e392e30383c2f74643e3c2f74723e0a202020203c74723e3c74",
    "683e323032383c2f74683e3c74643e34313c2f74643e3c74643e36392e35333c2f74643e3c74643e",
    "37362e30353c2f74643e3c74643e38352e30363c2f74643e3c74643e392e34303c2f74643e3c2f74",
    "723e0a20203c2f74626f64793e0a3c2f7461626c653e0a3c7461626c6520636c6173733d226d5f74",
    "61626c65206d5f686c223e0a20203c63617074696f6e3e3c7370616e3eb5a5cebba3bad2dad4aa3c",
    "2f7370616e3ebbe3d7dc2d2dd4a4b2e2c4eab1a8bebbc0fbc8f33c2f63617074696f6e3e0a20203c",
    "74686561643e3c74723e3c74683ec4eab6c83c2f74683e3c74683ed4a4b2e2bbfab9b9cafd3c2f74",
    "683e3c74683ed7eed0a1d6b53c2f74683e3c74683ebef9d6b53c2f74683e3c74683ed7eeb4f3d6b5",
    "3c2f74683e3c2f74723e3c2f74686561643e0a20203c74626f64793e3c74723e3c74683e32303236",
    "3c2f74683e3c74643e34363c2f74643e3c74643e3831322e37383c2f74643e3c74643e3836302e32",
    "343c2f74643e3c74643e3937342e38353c2f74643e3c2f74723e3c2f74626f64793e0a3c2f746162",
    "6c653e0a3c2f626f64793e0a3c2f68746d6c3e0a",
);
const EMPTY_HTML_GBK_HEX: &str = concat!(
    "3c68746d6c3e0a3c686561643e3c7469746c653ebbaab5e7c1c9c4dc2836303033393629bcdbd6b5",
    "b7d6cef63c2f7469746c653e3c2f686561643e0a3c626f64793e0a3c703ebdd8d6c1323032362d30",
    "372d3233a3acb1bec4eab6c8d4ddcedebbfab9b9d7f6b3f6d2b5bca8d4a4b2e23c2f703e0a3c2f62",
    "6f64793e0a3c2f68746d6c3e0a",
);

fn fixture_bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(digits, 16).unwrap()
        })
        .collect()
}

fn expected_request(url: &str) -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: url.to_owned(),
        headers: vec![
            (
                "User-Agent".to_owned(),
                "Mozilla/5.0 (compatible; magic-ths-rs/0.2; read-only public-data probe)"
                    .to_owned(),
            ),
            ("Accept".to_owned(), "text/html".to_owned()),
            (
                "Referer".to_owned(),
                "https://basic.10jqka.com.cn/".to_owned(),
            ),
        ],
    }
}

#[derive(Clone, Default)]
struct ConsensusHtmlFixture {
    attempts: Arc<Mutex<Vec<HttpRequest>>>,
}

impl ThsTransport for ConsensusHtmlFixture {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ThsError> {
        // Record every entry before endpoint/header/body guards can reject it.
        self.attempts
            .lock()
            .map_err(|_| ThsError::Transport("offline attempt trace poisoned".to_owned()))?
            .push(request.clone());
        let hex = match request.url.as_str() {
            COMPLETE_URL => COMPLETE_HTML_GBK_HEX,
            EMPTY_URL => EMPTY_HTML_GBK_HEX,
            _ => {
                return Err(ThsError::Transport(
                    "unexpected offline consensus endpoint".to_owned(),
                ));
            }
        };
        if request != &expected_request(&request.url) {
            return Err(ThsError::Transport(
                "unexpected offline consensus method or ordered headers".to_owned(),
            ));
        }
        let body = fixture_bytes(hex);
        if body.len() > 4 * 1024 * 1024 {
            return Err(ThsError::Transport(
                "offline consensus fixture exceeds source body bound".to_owned(),
            ));
        }
        Ok(HttpResponse {
            status: 200,
            final_url: request.url.clone(),
            content_type: Some("text/html; charset=GBK".to_owned()),
            body,
        })
    }
}

fn registry(fixture: ConsensusHtmlFixture) -> OperationRegistry {
    let config = ThsConfig::default();
    assert_eq!(config.minimum_interval, Duration::from_secs(1));
    let client = ThsClient::with_transport(config, fixture).unwrap();
    let mut registry = OperationRegistry::all_unadmitted("offline consensus public contract");
    // Private concrete registrar is setup only; observations use public Execute.
    register_ths_consensus_client(&mut registry, client, MAXIMUM_PAYLOAD_BYTES).unwrap();
    registry
}

fn command(request_id: &str, body: &[u8]) -> QueryCommand {
    let command = QueryCommand::new(
        request_id,
        Operation::Consensus,
        Some("Tonghuashun".to_owned()),
        CanonicalPayload::new(
            "magic.market.consensus.request",
            1,
            body.to_vec(),
            MAXIMUM_PAYLOAD_BYTES,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!command.allows_unadmitted());
    command
}

fn clock_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

#[test]
fn registered_consensus_does_not_publish_a_successful_prefix_when_later_source_is_empty() {
    let complete_bytes = fixture_bytes(COMPLETE_HTML_GBK_HEX);
    let empty_bytes = fixture_bytes(EMPTY_HTML_GBK_HEX);
    assert_eq!(complete_bytes.len(), 1020);
    assert_eq!(empty_bytes.len(), 133);
    assert_eq!(
        format!("{:x}", Sha256::digest(&complete_bytes)),
        "4dccdbeaedc42929ec3f4c725206f22c84f37b5ca48cbfedd17e6f8202fa4432"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&empty_bytes)),
        "96462842327023eb7d434e10b6650d50a57504c58c035a369e22fee576759d36"
    );

    let normal_fixture = ConsensusHtmlFixture::default();
    let normal_registry = registry(normal_fixture.clone());
    let before = clock_nanos();
    let result = normal_registry
        .execute(command(
            "ths-consensus-complete-control",
            br#"{"instruments":[{"exchange":"Shanghai","code":"600519","asset_class":"Equity"}]}"#,
        ))
        .unwrap();
    let after = clock_nanos();

    assert_eq!(result.provider, "Tonghuashun");
    assert!(result.complete);
    assert_eq!(result.source_at.as_deref(), Some("2026-07-23"));
    assert!(result.repository_admitted);
    assert_eq!(result.diagnostic_blocker, None);
    assert_eq!(result.records.len(), 1);
    assert_eq!(
        normal_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request(COMPLETE_URL)]
    );

    // Independently check the dynamic epoch-nanosecond observation, not just equality.
    let (seconds, fraction) = result.observed_at.split_once('.').unwrap();
    assert!(!seconds.is_empty());
    assert!(seconds.bytes().all(|byte| byte.is_ascii_digit()));
    assert_eq!(fraction.len(), 9);
    assert!(fraction.bytes().all(|byte| byte.is_ascii_digit()));
    let observed_nanos = seconds
        .parse::<u128>()
        .unwrap()
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(fraction.parse::<u128>().unwrap()))
        .unwrap();
    assert!(before <= observed_nanos && observed_nanos <= after);
    let expected_batch_id = format!("ths:{}:consensus", result.observed_at);
    assert_eq!(result.batch_id, expected_batch_id);

    let payload = &result.records[0];
    assert_eq!(payload.schema(), "magic.market.consensus_snapshot");
    assert_eq!(payload.schema_version(), 1);
    let row: serde_json::Value = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(row.as_object().unwrap().len(), 5);
    assert_eq!(row["estimates"].as_array().unwrap().len(), 3);
    for estimate in row["estimates"].as_array().unwrap() {
        assert_eq!(estimate.as_object().unwrap().len(), 7);
    }
    assert_eq!(row["evidence"].as_object().unwrap().len(), 4);
    let expected = serde_json::json!({
        "instrument": {
            "exchange": "Shanghai",
            "code": "600519",
            "asset_class": "Equity"
        },
        "name": "贵州茅台",
        "estimates": [
            {
                "fiscal_year": 2026, "eps": 68.73, "eps_min": 65.02,
                "eps_max": 77.85, "contributor_count": 46,
                "revenue": null, "profit": null
            },
            {
                "fiscal_year": 2027, "eps": 72.48, "eps_min": 67.61,
                "eps_max": 84.02, "contributor_count": 46,
                "revenue": null, "profit": null
            },
            {
                "fiscal_year": 2028, "eps": 76.05, "eps_min": 69.53,
                "eps_max": 85.06, "contributor_count": 41,
                "revenue": null, "profit": null
            }
        ],
        "contributor_count": null,
        "evidence": {
            "provider": "Tonghuashun",
            "source_at": "2026-07-23",
            "observed_at": result.observed_at.clone(),
            "batch_id": expected_batch_id.clone()
        }
    });
    assert_eq!(row, expected);
    let typed: ConsensusSnapshot = serde_json::from_slice(payload.data()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), expected);

    let rejection_fixture = ConsensusHtmlFixture::default();
    let rejection_registry = registry(rejection_fixture.clone());
    let error = rejection_registry
        .execute(command(
            "ths-consensus-later-source-empty",
            br#"{"instruments":[{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},{"exchange":"Shanghai","code":"600396","asset_class":"Equity"}]}"#,
        ))
        .unwrap_err();
    assert_eq!(
        error,
        ServiceError::InvalidEvidence {
            provider: "Tonghuashun".to_owned(),
            evidence_code: "consensus_provider_response_invalid".to_owned(),
            evidence_field: "consensus.provider_response".to_owned(),
            record_index: None,
            message: "Consensus rejected Tonghuashun evidence (consensus_provider_response_invalid at consensus.provider_response)"
                .to_owned(),
        }
    );
    assert_eq!(
        rejection_fixture.attempts.lock().unwrap().as_slice(),
        &[expected_request(COMPLETE_URL), expected_request(EMPTY_URL)]
    );
}
