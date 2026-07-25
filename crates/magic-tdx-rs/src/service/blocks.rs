//! Stable facade for industry, concept, and regional block data.

use crate::block::TdxBlockClient;
use crate::error::TdxError;
use crate::protocol::types::{IndexBar, SecurityQuote};
use crate::reader::block::BlockRecord;

/// Typed block-data service with the upstream client's safety limits.
pub struct BlockService {
    client: TdxBlockClient,
}

impl BlockService {
    /// Creates a block service for a TDX endpoint.
    pub fn new(ip: &str, port: u16, timeout: f64) -> Self {
        Self {
            client: TdxBlockClient::new(ip, port, timeout),
        }
    }
    /// Uses the default TDX port and timeout.
    pub fn with_default(ip: &str) -> Self {
        Self {
            client: TdxBlockClient::with_default(ip),
        }
    }
    /// Returns block K-lines with enforced category limits.
    pub fn bars(
        &self,
        category: u8,
        code: &str,
        start: u32,
        count: u16,
    ) -> Result<Vec<IndexBar>, TdxError> {
        self.client.get_block_bars(category, code, start, count)
    }
    /// Returns block quotes, preserving the requested code list.
    pub fn quotes(&self, codes: &[&str]) -> Result<Vec<SecurityQuote>, TdxError> {
        self.client.get_block_quotes(codes)
    }
    /// Loads industry block records.
    pub fn industry(&self) -> Result<Vec<BlockRecord>, TdxError> {
        self.client.get_industry_blocks()
    }
    /// Loads concept block records.
    pub fn concept(&self) -> Result<Vec<BlockRecord>, TdxError> {
        self.client.get_concept_blocks()
    }
    /// Loads index/region block records.
    pub fn index(&self) -> Result<Vec<BlockRecord>, TdxError> {
        self.client.get_index_blocks()
    }
    /// Updates the endpoint used by this service.
    pub fn set_server(&self, ip: &str, port: u16) {
        self.client.set_server(ip, port);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::KLINE_DAILY;

    #[test]
    fn block_facade_propagates_local_connection_failure() {
        let service = BlockService::new("127.0.0.1", 1, 0.01);
        service.set_server("127.0.0.1", 1);
        assert!(service.bars(KLINE_DAILY, "880001", 0, 5).is_err());
        assert!(service.quotes(&["880001"]).is_err());
        assert!(service.industry().is_err());
        assert!(service.concept().is_err());
        assert!(service.index().is_err());

        let default = BlockService::with_default("127.0.0.1");
        default.set_server("127.0.0.1", 1);
        assert!(default.quotes(&["880001"]).is_err());
    }
}
