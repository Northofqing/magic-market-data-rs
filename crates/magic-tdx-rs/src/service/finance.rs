//! Stable facade for realtime and report-file finance data.

use crate::net::finance_client::{GpcwFileInfo, TdxFinanceClient};
use crate::protocol::types::{FinanceInfo, XdXrInfo};
use crate::reader::financial::FinancialRecord;
use crate::TdxError;
use std::collections::HashMap;

/// Finance service, including the 34-field realtime and 45-field report APIs.
pub struct FinanceService {
    client: TdxFinanceClient,
}
impl FinanceService {
    pub fn new(ip: &str, port: u16, timeout: Option<f64>) -> Self {
        Self {
            client: TdxFinanceClient::new(ip, port, timeout),
        }
    }
    pub fn client(&self) -> &TdxFinanceClient {
        &self.client
    }
    pub fn info(&self, market: u8, code: &str) -> Result<FinanceInfo, TdxError> {
        self.client.get_finance_info(market, code)
    }
    pub fn corporate_actions(&self, market: u8, code: &str) -> Result<Vec<XdXrInfo>, TdxError> {
        self.client.get_xdxr_info(market, code)
    }
    pub fn files(&self) -> Result<Vec<GpcwFileInfo>, TdxError> {
        self.client.get_financial_list()
    }
    pub fn report(&self, filename: &str, filesize: u32) -> Result<Vec<u8>, TdxError> {
        self.client.get_report_file_by_size(filename, filesize)
    }
    pub fn records(&self, filename: &str, filesize: u32) -> Result<Vec<FinancialRecord>, TdxError> {
        self.client.get_financial_data(filename, filesize)
    }
    pub fn indicators(
        &self,
        filename: &str,
        filesize: u32,
        code: &str,
    ) -> Result<HashMap<&'static str, f64>, TdxError> {
        self.client.get_finance_indicators(filename, filesize, code)
    }
    pub fn labeled_indicators(
        &self,
        filename: &str,
        filesize: u32,
        code: &str,
    ) -> Result<Vec<(&'static str, &'static str, f64)>, TdxError> {
        self.client
            .get_finance_indicators_labeled(filename, filesize, code)
    }
}
