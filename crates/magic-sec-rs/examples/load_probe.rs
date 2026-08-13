use magic_market_core::{
    CompanyFilingRequest, CompanyFilingsProvider, PositiveU32, SecCompanyIdentity,
};
use magic_market_transport::{
    EndpointPolicy, HttpRequest, HttpResponse, HttpTransport, MediaType, ReqwestTransport,
    TransportError,
};
use magic_sec_rs::SecEdgarClient;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct MeasuredTransport {
    inner: ReqwestTransport,
    starts: Mutex<Vec<Instant>>,
    active: AtomicUsize,
    maximum_active: AtomicUsize,
}

impl HttpTransport for MeasuredTransport {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.starts
            .lock()
            .map_err(|_| TransportError::Internal("load-probe starts lock poisoned".into()))?
            .push(Instant::now());
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum_active.fetch_max(active, Ordering::SeqCst);
        let result = self.inner.execute(request);
        self.active.fetch_sub(1, Ordering::SeqCst);
        result
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let user_agent = std::env::var("SEC_USER_AGENT")
        .map_err(|_| "SEC_USER_AGENT must be application/version contact@example.com")?;
    let policy = EndpointPolicy::new(
        "data.sec.gov",
        vec!["/submissions".into()],
        vec![],
        vec![MediaType::Json],
        8 * 1024 * 1024,
        Duration::from_secs(15),
    )?;
    let measured = Arc::new(MeasuredTransport {
        inner: ReqwestTransport::new(policy)?,
        starts: Mutex::new(Vec::new()),
        active: AtomicUsize::new(0),
        maximum_active: AtomicUsize::new(0),
    });
    let client = SecEdgarClient::with_transport(user_agent, measured.clone())?;
    let request = CompanyFilingRequest::new(
        vec![SecCompanyIdentity::new("320193", Some("AAPL"))?],
        vec![],
        None,
        None,
        PositiveU32::new(1)?,
    )?;
    for _ in 0..3 {
        client.company_filings(&request)?;
    }

    let starts = measured.starts.lock().map_err(|_| "starts lock poisoned")?;
    if starts.len() != 3
        || starts
            .windows(2)
            .any(|pair| pair[1].duration_since(pair[0]) < Duration::from_millis(500))
        || measured.maximum_active.load(Ordering::SeqCst) != 1
    {
        return Err("SEC serial load contract failed".into());
    }
    println!(
        "requests={} maximum_concurrency={} minimum_spacing_ms=500",
        starts.len(),
        measured.maximum_active.load(Ordering::SeqCst)
    );
    Ok(())
}
