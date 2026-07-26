use magic_market_core::{
    Announcement, AssetClass, DataBatch, Exchange, HttpsUrl, InstrumentId, IsoDate,
    MarketAnnouncementRequest, MarketAnnouncements, NonEmptyText, PositiveU32, Provenance,
    ProviderId, SourceEvidence,
};
use magic_market_router::{
    market_announcement_source, AcceptancePolicy, AttemptStatus, FailureKind,
    MarketAnnouncementRouter, RoutedSource, SourceError,
};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("fixture failure")]
struct FixtureError;

struct FixtureProvider {
    batch: DataBatch<Announcement>,
}

impl MarketAnnouncements for FixtureProvider {
    type Error = FixtureError;

    fn market_announcements(
        &self,
        _request: &MarketAnnouncementRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error> {
        Ok(self.batch.clone())
    }
}

fn request(limit: u32) -> MarketAnnouncementRequest {
    MarketAnnouncementRequest::new(
        IsoDate::new("2026-07-24").unwrap(),
        IsoDate::new("2026-07-24").unwrap(),
        PositiveU32::new(limit).unwrap(),
    )
    .unwrap()
}

fn announcement(id: &str, batch_id: &str) -> Announcement {
    announcement_at(id, batch_id, "2026-07-24T08:00:00+08:00")
}

fn announcement_at(id: &str, batch_id: &str, published_at: &str) -> Announcement {
    Announcement {
        announcement_id: NonEmptyText::new(id).unwrap(),
        instrument: InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
        instrument_name: None,
        category: None,
        title: NonEmptyText::new("公告").unwrap(),
        published_at: NonEmptyText::new(published_at).unwrap(),
        canonical_url: HttpsUrl::new("https://www.cninfo.com.cn/a").unwrap(),
        pdf_url: None,
        evidence: SourceEvidence::new(ProviderId::Cninfo, "observed", batch_id)
            .unwrap()
            .with_source_at(published_at)
            .unwrap(),
    }
}

fn provider(records: Vec<Announcement>, batch_id: &str) -> Arc<FixtureProvider> {
    provider_at(records, Some(batch_id), Some("2026-07-24T08:00:00+08:00"))
}

fn provider_at(
    records: Vec<Announcement>,
    batch_id: Option<&str>,
    source_at: Option<&str>,
) -> Arc<FixtureProvider> {
    let mut provenance = Provenance::new("cninfo-market", "observed").unwrap();
    if let Some(source_at) = source_at {
        provenance = provenance.with_source_at(source_at).unwrap();
    }
    if let Some(batch_id) = batch_id {
        provenance = provenance.with_batch_id(batch_id).unwrap();
    }
    Arc::new(FixtureProvider {
        batch: DataBatch::strict(records, provenance),
    })
}

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Provider, "fixture failure")
}

#[test]
fn market_adapter_preserves_a_valid_whole_market_batch() {
    let source = market_announcement_source(
        ProviderId::Cninfo,
        provider(vec![announcement("ann-1", "batch-1")], "batch-1"),
        classify,
    );

    let batch = source.fetch(&request(1)).unwrap();

    assert_eq!(batch.records().len(), 1);
    assert_eq!(batch.records()[0].announcement_id.as_str(), "ann-1");
    assert_eq!(batch.provenance().source(), "cninfo-market");
}

fn empty_provider() -> Arc<FixtureProvider> {
    provider_at(Vec::new(), Some("empty-batch"), None)
}

#[test]
fn complete_empty_selection_is_explicit_and_default_off() {
    let mut default = MarketAnnouncementRouter::new(AcceptancePolicy::new());
    default
        .register(market_announcement_source(
            ProviderId::Cninfo,
            empty_provider(),
            classify,
        ))
        .unwrap();
    let rejected = default.route(&request(3)).unwrap_err();
    assert!(matches!(
        rejected.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::NoData,
            ..
        }
    ));

    let policy = AcceptancePolicy::new()
        .with_require_complete(true)
        .with_accept_complete_empty(true);
    let mut enabled = MarketAnnouncementRouter::new(policy);
    enabled
        .register(market_announcement_source(
            ProviderId::Cninfo,
            empty_provider(),
            classify,
        ))
        .unwrap();

    let selected = enabled.route(&request(3)).unwrap();

    assert_eq!(selected.selected_provider(), ProviderId::Cninfo);
    assert!(selected.batch().records().is_empty());
    assert!(selected.batch().quality().is_complete());
}

#[test]
fn market_adapter_rejects_duplicate_ids_and_record_evidence_drift() {
    let duplicate = market_announcement_source(
        ProviderId::Cninfo,
        provider(
            vec![
                announcement("ann-1", "batch-1"),
                announcement("ann-1", "batch-1"),
            ],
            "batch-1",
        ),
        classify,
    );
    assert_eq!(
        duplicate.fetch(&request(2)).unwrap_err().kind(),
        FailureKind::Quality
    );

    let mut drifted = announcement("ann-1", "batch-1");
    drifted.evidence = SourceEvidence::new(ProviderId::Cninfo, "observed", "batch-1")
        .unwrap()
        .with_source_at("2026-07-24T07:59:59+08:00")
        .unwrap();
    let evidence = market_announcement_source(
        ProviderId::Cninfo,
        provider(vec![drifted], "batch-1"),
        classify,
    );
    assert_eq!(
        evidence.fetch(&request(1)).unwrap_err().kind(),
        FailureKind::Evidence
    );
}

#[test]
fn market_adapter_rejects_batch_shape_and_completeness_violations() {
    let oversized = market_announcement_source(
        ProviderId::Cninfo,
        provider(
            vec![
                announcement("ann-1", "batch-1"),
                announcement("ann-2", "batch-1"),
            ],
            "batch-1",
        ),
        classify,
    );
    assert_eq!(
        oversized.fetch(&request(1)).unwrap_err().kind(),
        FailureKind::Quality
    );

    let no_batch_id = market_announcement_source(
        ProviderId::Cninfo,
        provider_at(
            vec![announcement("ann-1", "batch-1")],
            None,
            Some("2026-07-24T08:00:00+08:00"),
        ),
        classify,
    );
    assert_eq!(
        no_batch_id.fetch(&request(1)).unwrap_err().kind(),
        FailureKind::Evidence
    );

    let incomplete_empty = Arc::new(FixtureProvider {
        batch: DataBatch::best_effort(
            Vec::new(),
            Provenance::new("cninfo-market", "observed")
                .unwrap()
                .with_batch_id("empty-batch")
                .unwrap(),
            vec!["upstream pagination incomplete".into()],
        )
        .unwrap(),
    });
    let incomplete_empty =
        market_announcement_source(ProviderId::Cninfo, incomplete_empty, classify);
    assert_eq!(
        incomplete_empty.fetch(&request(1)).unwrap_err().kind(),
        FailureKind::Quality
    );

    let falsely_timestamped_empty = market_announcement_source(
        ProviderId::Cninfo,
        provider_at(
            Vec::new(),
            Some("empty-batch"),
            Some("2026-07-24T08:00:00+08:00"),
        ),
        classify,
    );
    assert_eq!(
        falsely_timestamped_empty
            .fetch(&request(1))
            .unwrap_err()
            .kind(),
        FailureKind::Evidence
    );
}

#[test]
fn market_adapter_rejects_provenance_and_identity_drift() {
    let wrong_batch_source_time = market_announcement_source(
        ProviderId::Cninfo,
        provider_at(
            vec![announcement("ann-1", "batch-1")],
            Some("batch-1"),
            Some("2026-07-24T07:59:59+08:00"),
        ),
        classify,
    );
    assert_eq!(
        wrong_batch_source_time
            .fetch(&request(1))
            .unwrap_err()
            .kind(),
        FailureKind::Evidence
    );

    let mut non_share = announcement("ann-1", "batch-1");
    non_share.instrument =
        InstrumentId::new(Exchange::Shanghai, "000001", AssetClass::Index).unwrap();
    let non_share = market_announcement_source(
        ProviderId::Cninfo,
        provider(vec![non_share], "batch-1"),
        classify,
    );
    assert_eq!(
        non_share.fetch(&request(1)).unwrap_err().kind(),
        FailureKind::Evidence
    );

    let mut wrong_provider = announcement("ann-1", "batch-1");
    wrong_provider.evidence = SourceEvidence::new(ProviderId::Tdx, "observed", "batch-1")
        .unwrap()
        .with_source_at("2026-07-24T08:00:00+08:00")
        .unwrap();
    let wrong_provider = market_announcement_source(
        ProviderId::Cninfo,
        provider(vec![wrong_provider], "batch-1"),
        classify,
    );
    assert_eq!(
        wrong_provider.fetch(&request(1)).unwrap_err().kind(),
        FailureKind::Evidence
    );

    let wrong_record_batch = market_announcement_source(
        ProviderId::Cninfo,
        provider(vec![announcement("ann-1", "record-batch")], "batch-1"),
        classify,
    );
    assert_eq!(
        wrong_record_batch.fetch(&request(1)).unwrap_err().kind(),
        FailureKind::Evidence
    );
}

#[test]
fn market_adapter_rejects_invalid_ranges_and_ordering() {
    for invalid_time in [
        "2026",
        "2026-07-24X08:00:00+08:00",
        "2026-02-30T08:00:00+08:00",
        "2026-07-23T08:00:00+08:00",
    ] {
        let source = market_announcement_source(
            ProviderId::Cninfo,
            provider_at(
                vec![announcement_at("ann-1", "batch-1", invalid_time)],
                Some("batch-1"),
                Some(invalid_time),
            ),
            classify,
        );
        assert_eq!(
            source.fetch(&request(1)).unwrap_err().kind(),
            FailureKind::Evidence,
            "{invalid_time}"
        );
    }

    let older = announcement_at("ann-1", "batch-1", "2026-07-24T07:59:00+08:00");
    let newer = announcement_at("ann-2", "batch-1", "2026-07-24T08:00:00+08:00");
    let unsorted = market_announcement_source(
        ProviderId::Cninfo,
        provider_at(
            vec![older, newer],
            Some("batch-1"),
            Some("2026-07-24T07:59:00+08:00"),
        ),
        classify,
    );
    assert_eq!(
        unsorted.fetch(&request(2)).unwrap_err().kind(),
        FailureKind::Quality
    );
}
