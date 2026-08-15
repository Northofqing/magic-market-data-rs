//! Strict provider-neutral company profiles backed by TDX public F10 data.

use std::collections::HashSet;

use magic_market_core::{
    AssetClass, CompanyCapabilities, DataBatch, Exchange, InstrumentId, IsoDate, NonEmptyText,
    ProfileFact, Provenance, ProviderId, SecurityMetadataProvider, SecurityProfile,
    SecurityProfiles, SourceEvidence,
};

use crate::{
    profile::{constants::category::COMPANY_PROFILE, types::F10Content},
    ProfileService, TdxError, TdxHqClient,
};

/// Repository admission for the exact normalized TDX company-overview scope.
///
/// This is promoted only after the bounded live/load evidence described by the
/// integration document has passed.
pub const SECURITY_PROFILES_ADMITTED: bool = true;
pub const MAX_SECURITY_PROFILE_INSTRUMENTS: usize = 8;
const MAX_PROFILE_FACTS: usize = 256;

/// Production candidate for strict TDX company-overview profiles.
pub struct TdxSecurityProfileProvider {
    metadata: TdxHqClient,
    profiles: ProfileService,
    timeout_seconds: f64,
}

impl TdxSecurityProfileProvider {
    pub fn new(ip: &str, port: u16, timeout_seconds: f64) -> Result<Self, TdxError> {
        if ip.trim().is_empty() {
            return Err(TdxError::InvalidData(
                "TDX security-profile server must not be empty".into(),
            ));
        }
        if port == 0 {
            return Err(TdxError::InvalidData(
                "TDX security-profile port must be positive".into(),
            ));
        }
        if !timeout_seconds.is_finite() || timeout_seconds <= 0.0 {
            return Err(TdxError::InvalidData(
                "TDX security-profile timeout must be finite and positive".into(),
            ));
        }
        let metadata = TdxHqClient::new();
        metadata.set_servers(&[("security-profile", ip, port)]);
        metadata.set_connect_timeout(timeout_seconds);
        Ok(Self {
            metadata,
            profiles: ProfileService::new(ip, port, timeout_seconds),
            timeout_seconds,
        })
    }

    pub const fn capabilities() -> CompanyCapabilities {
        CompanyCapabilities {
            security_profile: SECURITY_PROFILES_ADMITTED,
            balance_sheet: false,
            income_statement: false,
            cash_flow_statement: false,
        }
    }

    /// Named diagnostic path used only to gather admission evidence.
    pub fn diagnostic_security_profiles(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityProfile>, TdxError> {
        self.fetch(instruments)
    }

    fn fetch(&self, instruments: &[InstrumentId]) -> Result<DataBatch<SecurityProfile>, TdxError> {
        validate_request(instruments)?;
        self.metadata.connect_to_any(Some(self.timeout_seconds))?;
        let metadata = self.metadata.security_metadata(instruments)?;
        if metadata.records().len() != instruments.len() {
            return Err(TdxError::InvalidData(format!(
                "TDX security-profile metadata cardinality mismatch: requested {}, returned {}",
                instruments.len(),
                metadata.records().len()
            )));
        }

        let mut inputs = Vec::with_capacity(instruments.len());
        for (instrument, record) in instruments.iter().zip(metadata.records()) {
            if record.instrument() != instrument {
                return Err(TdxError::InvalidData(format!(
                    "TDX security-profile metadata identity mismatch for {}",
                    instrument.code()
                )));
            }
            let name = record.name().ok_or_else(|| {
                TdxError::InvalidData(format!(
                    "TDX security-profile metadata name is unavailable for {}",
                    instrument.code()
                ))
            })?;
            let market = market(instrument)?;
            let categories = self.profiles.categories(market, instrument.code())?;
            let matches = categories
                .iter()
                .filter(|category| category.name == COMPANY_PROFILE)
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(TdxError::InvalidData(format!(
                    "TDX security-profile requires exactly one {COMPANY_PROFILE:?} section for {}, found {}",
                    instrument.code(),
                    matches.len()
                )));
            }
            let content = self
                .profiles
                .content(market, instrument.code(), matches[0])?;
            inputs.push(ProfileInput {
                instrument: instrument.clone(),
                name: name.to_owned(),
                listed_on: record.listed_on().map(str::to_owned),
                content,
            });
        }
        normalize_profiles(inputs, observed_at()?)
    }
}

impl SecurityProfiles for TdxSecurityProfileProvider {
    type Error = TdxError;

    fn security_profiles(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<SecurityProfile>, Self::Error> {
        if !SECURITY_PROFILES_ADMITTED {
            return Err(TdxError::Unsupported(
                "TDX security profiles remain diagnostic until bounded live and load evidence passes"
                    .into(),
            ));
        }
        self.fetch(instruments)
    }
}

struct ProfileInput {
    instrument: InstrumentId,
    name: String,
    listed_on: Option<String>,
    content: F10Content,
}

fn normalize_profiles(
    inputs: Vec<ProfileInput>,
    observed_at: String,
) -> Result<DataBatch<SecurityProfile>, TdxError> {
    if inputs.is_empty() {
        return Err(TdxError::InvalidData(
            "TDX security-profile normalization received no records".into(),
        ));
    }
    let batch_id = format!("tdx-public-f10:{observed_at}");
    let provenance =
        Provenance::new("tdx-public-f10", observed_at.clone())?.with_batch_id(batch_id.clone())?;
    let evidence = SourceEvidence::new(ProviderId::Tdx, observed_at, batch_id)?;
    let mut records = Vec::with_capacity(inputs.len());
    for input in inputs {
        if input.content.category != COMPANY_PROFILE {
            return Err(TdxError::InvalidData(format!(
                "TDX security-profile content category mismatch for {}: {:?}",
                input.instrument.code(),
                input.content.category
            )));
        }
        let facts = profile_facts(&input.content)?;
        records.push(SecurityProfile {
            instrument: input.instrument,
            name: NonEmptyText::new(input.name)?,
            industry: None,
            listed_on: input.listed_on.map(IsoDate::new).transpose()?,
            total_shares: None,
            floating_shares: None,
            facts,
            evidence: evidence.clone(),
        });
    }
    Ok(DataBatch::strict(records, provenance))
}

fn profile_facts(content: &F10Content) -> Result<Vec<ProfileFact>, TdxError> {
    let mut facts = Vec::new();
    for (index, line) in content.content.lines().enumerate() {
        let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            continue;
        }
        if facts.len() == MAX_PROFILE_FACTS {
            return Err(TdxError::InvalidData(format!(
                "TDX {COMPANY_PROFILE} section exceeds {MAX_PROFILE_FACTS} non-empty lines"
            )));
        }
        facts.push(ProfileFact {
            key: NonEmptyText::new(format!("company_overview_line_{:03}", index + 1))?,
            source_label: NonEmptyText::new(content.category.clone())?,
            value: NonEmptyText::new(normalized)?,
        });
    }
    if facts.is_empty() {
        return Err(TdxError::InvalidData(format!(
            "TDX {COMPANY_PROFILE} section is empty"
        )));
    }
    Ok(facts)
}

fn validate_request(instruments: &[InstrumentId]) -> Result<(), TdxError> {
    if instruments.is_empty() || instruments.len() > MAX_SECURITY_PROFILE_INSTRUMENTS {
        return Err(TdxError::InvalidData(format!(
            "TDX security-profile request requires 1..={MAX_SECURITY_PROFILE_INSTRUMENTS} instruments"
        )));
    }
    let mut seen = HashSet::with_capacity(instruments.len());
    for instrument in instruments {
        if instrument.asset_class() != AssetClass::Equity {
            return Err(TdxError::Unsupported(format!(
                "TDX security profiles support equities only: {}",
                instrument.code()
            )));
        }
        if !matches!(
            instrument.exchange(),
            Exchange::Shanghai | Exchange::Shenzhen
        ) {
            return Err(TdxError::Unsupported(format!(
                "TDX security profiles support Shanghai and Shenzhen only: {}",
                instrument.code()
            )));
        }
        if !seen.insert(instrument.clone()) {
            return Err(TdxError::InvalidData(format!(
                "TDX security-profile request contains duplicate instrument {}",
                instrument.code()
            )));
        }
    }
    Ok(())
}

fn market(instrument: &InstrumentId) -> Result<u8, TdxError> {
    match instrument.exchange() {
        Exchange::Shanghai => Ok(1),
        Exchange::Shenzhen => Ok(0),
        Exchange::Beijing => Err(TdxError::Unsupported(
            "TDX F10 company profiles do not have verified Beijing identity".into(),
        )),
    }
}

fn observed_at() -> Result<String, TdxError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos()))
        .map_err(|error| {
            TdxError::InvalidData(format!("system clock is before UNIX epoch: {error}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh() -> InstrumentId {
        InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
    }

    #[test]
    fn request_is_bounded_unique_and_equity_only() {
        assert!(validate_request(&[]).is_err());
        assert!(validate_request(&[sh(), sh()]).is_err());
        let index = InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).unwrap();
        assert!(matches!(
            validate_request(&[index]),
            Err(TdxError::Unsupported(_))
        ));
        let beijing = InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap();
        assert!(matches!(
            validate_request(&[beijing]),
            Err(TdxError::Unsupported(_))
        ));
    }

    #[test]
    fn normalization_preserves_source_lines_without_inventing_fields() {
        let batch = normalize_profiles(
            vec![ProfileInput {
                instrument: sh(),
                name: "华电辽能".into(),
                listed_on: Some("1998-07-01".into()),
                content: F10Content::new(
                    COMPANY_PROFILE.into(),
                    " 公司名称  华电辽宁能源发展股份有限公司\n所属行业 电力 \n".into(),
                ),
            }],
            "1770000000.000000001".into(),
        )
        .unwrap();
        assert!(batch.quality().is_complete());
        assert_eq!(batch.records().len(), 1);
        let profile = &batch.records()[0];
        assert_eq!(profile.name.as_str(), "华电辽能");
        assert_eq!(profile.listed_on.as_ref().unwrap().as_str(), "1998-07-01");
        assert!(profile.industry.is_none());
        assert!(profile.total_shares.is_none());
        assert_eq!(profile.facts.len(), 2);
        assert_eq!(
            profile.facts[0].value.as_str(),
            "公司名称 华电辽宁能源发展股份有限公司"
        );
        assert_eq!(profile.evidence.provider(), ProviderId::Tdx);
        assert!(profile.evidence.source_at().is_none());
    }

    #[test]
    fn normalization_rejects_empty_wrong_or_oversized_facts() {
        let empty = F10Content::new(COMPANY_PROFILE.into(), " \n\t".into());
        assert!(profile_facts(&empty).is_err());
        let wrong = F10Content::new("财务分析".into(), "line".into());
        assert!(normalize_profiles(
            vec![ProfileInput {
                instrument: sh(),
                name: "华电辽能".into(),
                listed_on: None,
                content: wrong,
            }],
            "1770000000.000000001".into(),
        )
        .is_err());
        let oversized = F10Content::new(COMPANY_PROFILE.into(), "x\n".repeat(257));
        assert!(profile_facts(&oversized).is_err());
    }

    #[test]
    fn formal_trait_is_admitted_but_invalid_requests_still_fail_before_io() {
        let provider = TdxSecurityProfileProvider::new("127.0.0.1", 1, 0.01).unwrap();
        assert!(TdxSecurityProfileProvider::capabilities().security_profile);
        assert!(matches!(
            provider.security_profiles(&[]),
            Err(TdxError::InvalidData(_))
        ));
    }
}
