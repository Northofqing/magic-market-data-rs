use magic_exchange_rs::{
    CffexClient, CffexConfig, CffexTlsBackend, ExchangeError, ExchangeTransport, HttpRequest,
    HttpResponse, MAX_RESPONSE_BYTES,
};
use magic_market_core::{FuturesDeliveryRequest, PositiveU32};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const LIST: &str = r#"
  <a href="/cn/jystz/20260224/46999.html">
  关于股指期货和股指期权合约交割的通知</a><span>2026-02-23</span>
"#;
const DETAIL: &str = r#"
  <h1>关于股指期货和股指期权合约交割的通知</h1>
  <p>IF2602等合约于2026年2月24日进行交割，各合约的交割结算价具体如下：</p>
  <p>IF2602 IC2602 IM2602 IH2602 合约交割结算价。</p>
"#;

#[derive(Clone)]
struct Scripted {
    responses: Arc<Mutex<VecDeque<Result<HttpResponse, ExchangeError>>>>,
    starts: Arc<Mutex<Vec<Instant>>>,
}

impl Scripted {
    fn new(responses: Vec<Result<HttpResponse, ExchangeError>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into())),
            starts: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ExchangeTransport for Scripted {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ExchangeError> {
        self.starts.lock().unwrap().push(Instant::now());
        let mut response = self.responses.lock().unwrap().pop_front().unwrap()?;
        if response.final_url.is_empty() {
            response.final_url = request.url.clone();
        }
        Ok(response)
    }
}

fn response(body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        final_url: String::new(),
        content_type: Some("text/html; charset=UTF-8".into()),
        body: body.as_bytes().to_vec(),
    }
}

fn request() -> FuturesDeliveryRequest {
    FuturesDeliveryRequest::new(
        PositiveU32::new(2026).unwrap(),
        PositiveU32::new(2).unwrap(),
    )
    .unwrap()
}

#[test]
fn tls_backend_is_explicit_and_never_silently_falls_back() {
    let client = CffexClient::with_config(CffexConfig {
        tls_backend: CffexTlsBackend::Rustls,
        ..CffexConfig::default()
    })
    .unwrap();
    assert_eq!(client.tls_backend(), CffexTlsBackend::Rustls);
    assert!(format!("{client:?}").contains(CffexTlsBackend::Rustls.as_str()));
}

#[cfg(feature = "native-tls")]
#[test]
fn native_tls_backend_is_available_only_when_explicitly_compiled() {
    let client = CffexClient::with_config(CffexConfig {
        tls_backend: CffexTlsBackend::NativeTls,
        ..CffexConfig::default()
    })
    .unwrap();
    assert_eq!(client.tls_backend(), CffexTlsBackend::NativeTls);
    assert!(format!("{client:?}").contains(CffexTlsBackend::NativeTls.as_str()));
}

#[cfg(not(feature = "native-tls"))]
#[test]
fn native_tls_selection_fails_explicitly_when_feature_is_disabled() {
    assert!(matches!(
        CffexClient::with_config(CffexConfig {
            tls_backend: CffexTlsBackend::NativeTls,
            ..CffexConfig::default()
        }),
        Err(ExchangeError::Unsupported(message))
            if message.contains("feature native-tls")
    ));
}

#[test]
fn config_rejects_non_allowlisted_endpoint_timeout_and_pacing() {
    for config in [
        CffexConfig {
            list_endpoint: "https://example.com/jystz/".into(),
            ..CffexConfig::default()
        },
        CffexConfig {
            timeout: Duration::ZERO,
            ..CffexConfig::default()
        },
        CffexConfig {
            timeout: Duration::from_secs(61),
            ..CffexConfig::default()
        },
        CffexConfig {
            minimum_interval: Duration::from_millis(999),
            ..CffexConfig::default()
        },
    ] {
        assert!(matches!(
            CffexClient::with_transport(config, Scripted::new(vec![])),
            Err(ExchangeError::InvalidRequest(_))
        ));
    }
}

#[test]
fn response_contract_rejects_redirect_mime_and_oversized_body() {
    let cases = [
        HttpResponse {
            status: 200,
            final_url: "https://www.cffex.com.cn/cn/jystz_2.html".into(),
            content_type: Some("text/html".into()),
            body: LIST.as_bytes().to_vec(),
        },
        HttpResponse {
            status: 200,
            final_url: String::new(),
            content_type: Some("application/json".into()),
            body: LIST.as_bytes().to_vec(),
        },
        HttpResponse {
            status: 200,
            final_url: String::new(),
            content_type: Some("text/html".into()),
            body: vec![b'x'; MAX_RESPONSE_BYTES + 1],
        },
    ];
    for response in cases {
        let client =
            CffexClient::with_transport(CffexConfig::default(), Scripted::new(vec![Ok(response)]))
                .unwrap();
        assert!(client.probe_futures_delivery_calendar(&request()).is_err());
    }
}

#[test]
fn typed_tls_failure_preserves_the_selected_backend() {
    let transport = Scripted::new(vec![Err(ExchangeError::Tls {
        backend: CffexTlsBackend::NativeTls,
        message: "fixture handshake EOF".into(),
    })]);
    let client = CffexClient::with_transport(
        CffexConfig {
            tls_backend: CffexTlsBackend::NativeTls,
            ..CffexConfig::default()
        },
        transport,
    )
    .unwrap();
    assert!(matches!(
        client.probe_futures_delivery_calendar(&request()),
        Err(ExchangeError::Tls {
            backend: CffexTlsBackend::NativeTls,
            message,
        }) if message == "fixture handshake EOF"
    ));
}

#[test]
fn clone_shared_gate_spaces_actual_transport_starts() {
    let transport = Scripted::new(vec![Ok(response(LIST)), Ok(response(DETAIL))]);
    let starts = Arc::clone(&transport.starts);
    let client = CffexClient::with_transport(CffexConfig::default(), transport).unwrap();
    assert_eq!(
        client
            .probe_futures_delivery_calendar(&request())
            .unwrap()
            .records()
            .len(),
        4
    );
    let starts = starts.lock().unwrap();
    assert_eq!(starts.len(), 2);
    assert!(starts[1].duration_since(starts[0]) >= Duration::from_millis(950));
}
