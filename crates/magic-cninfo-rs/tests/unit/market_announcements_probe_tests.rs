use super::validate_probe_batch;
use magic_market_core::{
    Announcement, AssetClass, DataBatch, Exchange, HttpsUrl, InstrumentId, IsoDate,
    MarketAnnouncementRequest, NonEmptyText, PositiveU32, ProbeStatus, Provenance, ProviderId,
    SourceEvidence,
};

fn request() -> MarketAnnouncementRequest {
    MarketAnnouncementRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(3).unwrap(),
    )
    .unwrap()
}

#[test]
fn probe_admits_normalized_records_and_exact_verified_empty() {
    let published = "2026-07-24T08:00:00+08:00";
    let record = Announcement {
        announcement_id: NonEmptyText::new("ann-1").unwrap(),
        instrument: InstrumentId::new(Exchange::Beijing, "920189", AssetClass::Equity).unwrap(),
        category: None,
        title: NonEmptyText::new("公告").unwrap(),
        published_at: NonEmptyText::new(published).unwrap(),
        canonical_url: HttpsUrl::new("https://www.cninfo.com.cn/a").unwrap(),
        pdf_url: None,
        evidence: SourceEvidence::new(ProviderId::Cninfo, "observed", "batch")
            .unwrap()
            .with_source_at(published)
            .unwrap(),
    };
    let provenance = Provenance::new("cninfo-market", "observed")
        .unwrap()
        .with_source_at(published)
        .unwrap()
        .with_batch_id("batch")
        .unwrap();
    assert_eq!(
        validate_probe_batch(&DataBatch::strict(vec![record], provenance), &request()).unwrap(),
        ProbeStatus::Admitted
    );

    let empty = DataBatch::strict(
        Vec::new(),
        Provenance::new("cninfo-market", "observed")
            .unwrap()
            .with_batch_id("pages=1:total=0")
            .unwrap(),
    );
    assert_eq!(
        validate_probe_batch(&empty, &request()).unwrap(),
        ProbeStatus::VerifiedEmpty
    );
}
