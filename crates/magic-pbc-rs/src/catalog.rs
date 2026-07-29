use crate::PbcError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PbcTableDescriptor {
    year: u16,
    namespace: &'static str,
    canonical_url: &'static str,
    title_zh: &'static str,
    title_en: &'static str,
    unit_zh: &'static str,
    unit_en: &'static str,
}

impl PbcTableDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        year: u16,
        namespace: &'static str,
        canonical_url: &'static str,
        title_zh: &'static str,
        title_en: &'static str,
        unit_zh: &'static str,
        unit_en: &'static str,
    ) -> Result<Self, PbcError> {
        if !(1900..=9999).contains(&year)
            || namespace != "money-supply"
            || title_zh != "货币供应量"
            || title_en != "Money Supply"
            || unit_zh != "亿元人民币"
            || unit_en != "100 Million Yuan"
        {
            return Err(PbcError::InvalidRequest(
                "PBC table descriptor facts differ from the audited money-supply family".into(),
            ));
        }
        let expected_prefix =
            format!("https://www.pbc.gov.cn/eportal/fileDir/diaochatongjisi/resource/cms/{year}/");
        if !canonical_url.starts_with(&expected_prefix) || !canonical_url.ends_with(".htm") {
            return Err(PbcError::InvalidRequest(
                "PBC descriptor URL must be an exact official HTML path for its year".into(),
            ));
        }
        Ok(Self {
            year,
            namespace,
            canonical_url,
            title_zh,
            title_en,
            unit_zh,
            unit_en,
        })
    }

    pub const fn year(self) -> u16 {
        self.year
    }
    pub const fn namespace(self) -> &'static str {
        self.namespace
    }
    pub const fn canonical_url(self) -> &'static str {
        self.canonical_url
    }
    pub const fn title_zh(self) -> &'static str {
        self.title_zh
    }
    pub const fn title_en(self) -> &'static str {
        self.title_en
    }
    pub const fn unit_zh(self) -> &'static str {
        self.unit_zh
    }
    pub const fn unit_en(self) -> &'static str {
        self.unit_en
    }
}

const MONEY_SUPPLY_2024: PbcTableDescriptor = PbcTableDescriptor {
    year: 2024,
    namespace: "money-supply",
    canonical_url:
        "https://www.pbc.gov.cn/eportal/fileDir/diaochatongjisi/resource/cms/2024/11/2024111416041159339.htm",
    title_zh: "货币供应量",
    title_en: "Money Supply",
    unit_zh: "亿元人民币",
    unit_en: "100 Million Yuan",
};

pub fn descriptor_for_year(year: u16) -> Result<&'static PbcTableDescriptor, PbcError> {
    match year {
        2024 => Ok(&MONEY_SUPPLY_2024),
        _ => Err(PbcError::Unsupported(format!(
            "PBC money-supply year {year} is not in the audited official HTML catalog"
        ))),
    }
}
