#![forbid(unsafe_code)]
//! Concrete provider-to-Router bindings.
//!
//! Core and Router remain provider-neutral. This crate is the explicit
//! composition boundary where a route may require a concrete provider type so
//! downstream wrappers cannot impersonate an admitted source.

mod eastmoney_provider_top_n_rankings;

pub use eastmoney_provider_top_n_rankings::{
    EastmoneyProviderTopNRankingRouter, EastmoneyProviderTopNRouterError,
};
