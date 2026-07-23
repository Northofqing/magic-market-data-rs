use crate::SourceError;
use magic_market_core::{DataBatch, ProviderId};
use std::sync::Arc;

/// Object-safe provider operation used by one failover chain.
pub trait RoutedSource<Request: ?Sized, Record>: Send + Sync {
    fn provider_id(&self) -> ProviderId;
    fn fetch(&self, request: &Request) -> Result<DataBatch<Record>, SourceError>;
}

type FetchFn<Request, Record> =
    dyn Fn(&Request) -> Result<DataBatch<Record>, SourceError> + Send + Sync;

/// Closure-backed source adapter for deterministic and concrete providers.
pub struct SourceFn<Request: ?Sized, Record> {
    provider_id: ProviderId,
    fetch: Arc<FetchFn<Request, Record>>,
}

impl<Request: ?Sized, Record> SourceFn<Request, Record> {
    pub fn new<F>(provider_id: ProviderId, fetch: F) -> Self
    where
        F: Fn(&Request) -> Result<DataBatch<Record>, SourceError> + Send + Sync + 'static,
    {
        Self {
            provider_id,
            fetch: Arc::new(fetch),
        }
    }
}

impl<Request: ?Sized, Record> std::fmt::Debug for SourceFn<Request, Record> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceFn")
            .field("provider_id", &self.provider_id)
            .finish_non_exhaustive()
    }
}

impl<Request: ?Sized, Record> RoutedSource<Request, Record> for SourceFn<Request, Record> {
    fn provider_id(&self) -> ProviderId {
        self.provider_id
    }

    fn fetch(&self, request: &Request) -> Result<DataBatch<Record>, SourceError> {
        (self.fetch)(request)
    }
}
