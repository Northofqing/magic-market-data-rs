use magic_market_core::{
    Announcement, Announcements, AssetClass, ContractMonth, DataBatch, DragonTigerData,
    DragonTigerDisclosure, DragonTigerEntry, DragonTigerSeat, DragonTigerSide, Exchange,
    FiniteNumber, HttpsUrl, InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest,
    IsoDate, MarketDragonTigerData, MarketDragonTigerRequest, MarketStatistics,
    MarketStatisticsProvider, Money, NewsItem, NewsProvider, NonEmptyText, NorthboundChannel,
    NorthboundDailyRequest, NorthboundDailyStat, NorthboundDailyStatistics, NorthboundQuotaBalance,
    NorthboundTopTurnover, OptionContract, OptionData, OptionGreeks, OptionKind, OptionQuote,
    PositiveU32, PostCloseFlow, PostCloseFlowRequest, PostCloseFlows, Price, Provenance,
    ProviderId, Quantity, Ratio, RatioUnit, SourceEvidence,
};
use magic_market_router::{
    announcement_source, dragon_tiger_entry_source, dragon_tiger_seat_source, global_news_source,
    market_dragon_tiger_source, market_statistics_source, northbound_daily_source,
    option_contract_source, option_greeks_source, option_quote_source, post_close_flow_source,
    AcceptancePolicy, AnnouncementRouter, AttemptStatus, DragonTigerEntryRouter,
    DragonTigerSeatRouter, FailureKind, GlobalNewsRouter, MarketDragonTigerRouter,
    MarketStatisticsRouter, NorthboundDailyRouter, OptionContractRouter, OptionGreeksRouter,
    OptionQuoteRouter, PostCloseFlowRouter, RoutedSource, SourceError,
};
use std::sync::{Arc, Mutex};

#[derive(Debug, thiserror::Error)]
#[error("fixture")]
struct FixtureError;

struct FixtureProvider {
    record_provider: ProviderId,
    record_batch: &'static str,
    batch_provider_name: &'static str,
    batch_id: &'static str,
}

impl MarketStatisticsProvider for FixtureProvider {
    type Error = FixtureError;

    fn market_statistics(
        &self,
        instruments: &[InstrumentId],
    ) -> Result<DataBatch<MarketStatistics>, Self::Error> {
        let record = MarketStatistics::new(
            instruments[0].clone(),
            None,
            None,
            None,
            None,
            Some(Money::new(1.0).unwrap()),
            None,
            None,
            None,
            None,
            SourceEvidence::new(self.record_provider, "observed", self.record_batch).unwrap(),
        )
        .unwrap();
        Ok(DataBatch::strict(
            vec![record],
            Provenance::new(self.batch_provider_name, "observed")
                .unwrap()
                .with_batch_id(self.batch_id)
                .unwrap(),
        ))
    }
}

fn instrument() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap()
}

fn classify(_: FixtureError) -> SourceError {
    SourceError::try_next(FailureKind::Transport, "fixture")
}

struct NewsFixtureProvider {
    record_provider: ProviderId,
    batch_source: &'static str,
    item_count: usize,
    duplicate_id: bool,
}

impl NewsProvider for NewsFixtureProvider {
    type Error = FixtureError;

    fn instrument_news(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<NewsItem>, Self::Error> {
        unreachable!("global-news routing does not call instrument_news")
    }

    fn global_news(&self, _limit: PositiveU32) -> Result<DataBatch<NewsItem>, Self::Error> {
        let batch_id = format!("{}-news", self.batch_source);
        let published_at = "2026-07-24T20:00:00+08:00";
        let records = (0..self.item_count)
            .map(|index| {
                let item_id = if self.duplicate_id && index > 0 {
                    "news-1".to_owned()
                } else {
                    format!("news-{}", index + 1)
                };
                let evidence = SourceEvidence::new(self.record_provider, "observed", &batch_id)
                    .unwrap()
                    .with_source_at(published_at)
                    .unwrap();
                NewsItem {
                    item_id: NonEmptyText::new(item_id.clone()).unwrap(),
                    title: NonEmptyText::new("fixture financial news").unwrap(),
                    summary: None,
                    content: None,
                    publisher: NonEmptyText::new(self.batch_source).unwrap(),
                    canonical_url: HttpsUrl::new(format!("https://example.com/news/{item_id}"))
                        .unwrap(),
                    published_at: NonEmptyText::new(published_at).unwrap(),
                    instruments: Vec::new(),
                    topics: Vec::new(),
                    language: NonEmptyText::new("zh-CN").unwrap(),
                    evidence,
                }
            })
            .collect();
        Ok(DataBatch::strict(
            records,
            Provenance::new(self.batch_source, "observed")
                .unwrap()
                .with_source_at(published_at)
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        ))
    }
}

#[test]
fn global_news_router_preserves_jin10_and_thepaper_identities() {
    let wrong = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::ThePaper,
        batch_source: "jin10-v1",
        item_count: 1,
        duplicate_id: false,
    });
    let valid = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::ThePaper,
        batch_source: "thepaper-finance-v1",
        item_count: 1,
        duplicate_id: false,
    });
    let mut router = GlobalNewsRouter::new(AcceptancePolicy::new().with_require_source_at(true));
    router
        .register(global_news_source(ProviderId::Jin10, wrong, classify))
        .unwrap();
    router
        .register(global_news_source(ProviderId::ThePaper, valid, classify))
        .unwrap();

    let outcome = router.route(&PositiveU32::new(5).unwrap()).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::ThePaper);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Selected
    ));
}

#[test]
fn global_news_router_accepts_eastmoney_identity() {
    let provider = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::Eastmoney,
        batch_source: "eastmoney-web",
        item_count: 2,
        duplicate_id: false,
    });
    let mut router = GlobalNewsRouter::new(AcceptancePolicy::new().with_require_source_at(true));
    router
        .register(global_news_source(
            ProviderId::Eastmoney,
            provider,
            classify,
        ))
        .unwrap();

    let outcome = router.route(&PositiveU32::new(2).unwrap()).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Selected
    ));
}

#[test]
fn global_news_router_accepts_yonhap_identity() {
    let provider = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::Yonhap,
        batch_source: "yonhap-cn-rss-v1",
        item_count: 2,
        duplicate_id: false,
    });
    let mut router = GlobalNewsRouter::new(AcceptancePolicy::new().with_require_source_at(true));
    router
        .register(global_news_source(ProviderId::Yonhap, provider, classify))
        .unwrap();

    let outcome = router.route(&PositiveU32::new(2).unwrap()).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Yonhap);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Selected
    ));
}

#[test]
fn global_news_router_rejects_yonhap_identity_mismatch() {
    let wrong = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::ThePaper,
        batch_source: "yonhap-cn-rss-v1",
        item_count: 1,
        duplicate_id: false,
    });
    let valid = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::ThePaper,
        batch_source: "thepaper-finance-v1",
        item_count: 1,
        duplicate_id: false,
    });
    let mut router = GlobalNewsRouter::new(AcceptancePolicy::new().with_require_source_at(true));
    router
        .register(global_news_source(ProviderId::Yonhap, wrong, classify))
        .unwrap();
    router
        .register(global_news_source(ProviderId::ThePaper, valid, classify))
        .unwrap();

    let outcome = router.route(&PositiveU32::new(1).unwrap()).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::ThePaper);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn global_news_router_accepts_wallstreetcn_identity() {
    let provider = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::WallstreetCn,
        batch_source: "wallstreetcn-rss-v1",
        item_count: 2,
        duplicate_id: false,
    });
    let mut router = GlobalNewsRouter::new(AcceptancePolicy::new().with_require_source_at(true));
    router
        .register(global_news_source(
            ProviderId::WallstreetCn,
            provider,
            classify,
        ))
        .unwrap();

    let outcome = router.route(&PositiveU32::new(2).unwrap()).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::WallstreetCn);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Selected
    ));
}

#[test]
fn global_news_router_rejects_wallstreetcn_identity_mismatch() {
    let wrong = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::ThePaper,
        batch_source: "wallstreetcn-rss-v1",
        item_count: 1,
        duplicate_id: false,
    });
    let valid = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::ThePaper,
        batch_source: "thepaper-finance-v1",
        item_count: 1,
        duplicate_id: false,
    });
    let mut router = GlobalNewsRouter::new(AcceptancePolicy::new().with_require_source_at(true));
    router
        .register(global_news_source(
            ProviderId::WallstreetCn,
            wrong,
            classify,
        ))
        .unwrap();
    router
        .register(global_news_source(ProviderId::ThePaper, valid, classify))
        .unwrap();

    let outcome = router.route(&PositiveU32::new(1).unwrap()).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::ThePaper);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn global_news_router_rejects_oversized_and_duplicate_batches() {
    let valid = || {
        Arc::new(NewsFixtureProvider {
            record_provider: ProviderId::ThePaper,
            batch_source: "thepaper-finance-v1",
            item_count: 1,
            duplicate_id: false,
        })
    };

    let mut oversized_router = GlobalNewsRouter::new(AcceptancePolicy::new());
    oversized_router
        .register(global_news_source(
            ProviderId::Jin10,
            Arc::new(NewsFixtureProvider {
                record_provider: ProviderId::Jin10,
                batch_source: "jin10-v1",
                item_count: 2,
                duplicate_id: false,
            }),
            classify,
        ))
        .unwrap();
    oversized_router
        .register(global_news_source(ProviderId::ThePaper, valid(), classify))
        .unwrap();
    let oversized = oversized_router
        .route(&PositiveU32::new(1).unwrap())
        .unwrap();
    assert_eq!(oversized.selected_provider(), ProviderId::ThePaper);
    assert!(matches!(
        oversized.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));

    let mut duplicate_router = GlobalNewsRouter::new(AcceptancePolicy::new());
    duplicate_router
        .register(global_news_source(
            ProviderId::Jin10,
            Arc::new(NewsFixtureProvider {
                record_provider: ProviderId::Jin10,
                batch_source: "jin10-v1",
                item_count: 2,
                duplicate_id: true,
            }),
            classify,
        ))
        .unwrap();
    duplicate_router
        .register(global_news_source(ProviderId::ThePaper, valid(), classify))
        .unwrap();
    let duplicate = duplicate_router
        .route(&PositiveU32::new(2).unwrap())
        .unwrap();
    assert_eq!(duplicate.selected_provider(), ProviderId::ThePaper);
    assert!(matches!(
        duplicate.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

struct AnnouncementFixtureProvider {
    record_provider: ProviderId,
    batch_source: &'static str,
    instrument: InstrumentId,
    published_at: &'static str,
    duplicate_id: bool,
}

impl Announcements for AnnouncementFixtureProvider {
    type Error = FixtureError;

    fn announcements(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error> {
        let batch_id = format!("announcement-{}", self.batch_source);
        let evidence = SourceEvidence::new(self.record_provider, "observed", &batch_id)
            .unwrap()
            .with_source_at(self.published_at)
            .unwrap();
        let record = Announcement {
            announcement_id: NonEmptyText::new("A001").unwrap(),
            instrument: self.instrument.clone(),
            instrument_name: Some(NonEmptyText::new("fixture stock").unwrap()),
            category: None,
            title: NonEmptyText::new("fixture announcement").unwrap(),
            published_at: NonEmptyText::new(self.published_at).unwrap(),
            canonical_url: HttpsUrl::new("https://example.com/announcement/A001").unwrap(),
            pdf_url: None,
            evidence,
        };
        let records = if self.duplicate_id {
            vec![record.clone(), record]
        } else {
            vec![record]
        };
        Ok(DataBatch::strict(
            records,
            Provenance::new(self.batch_source, "observed")
                .unwrap()
                .with_source_at(self.published_at)
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        ))
    }
}

#[test]
fn announcement_adapter_rejects_wrong_identity_date_and_duplicate_ids() {
    let requested = instrument();
    let request = InstrumentDateRangeRequest::new(requested.clone(), PositiveU32::new(2).unwrap())
        .unwrap()
        .with_range(
            IsoDate::new("2026-07-01").unwrap(),
            IsoDate::new("2026-07-23").unwrap(),
        )
        .unwrap();
    let providers = [
        (
            ProviderId::Sse,
            AnnouncementFixtureProvider {
                record_provider: ProviderId::Sse,
                batch_source: "sse",
                instrument: InstrumentId::new(Exchange::Shanghai, "600000", AssetClass::Equity)
                    .unwrap(),
                published_at: "2026-07-23",
                duplicate_id: false,
            },
        ),
        (
            ProviderId::Szse,
            AnnouncementFixtureProvider {
                record_provider: ProviderId::Szse,
                batch_source: "szse",
                instrument: requested.clone(),
                published_at: "2026-06-30",
                duplicate_id: false,
            },
        ),
        (
            ProviderId::Tencent,
            AnnouncementFixtureProvider {
                record_provider: ProviderId::Tencent,
                batch_source: "tencent",
                instrument: requested.clone(),
                published_at: "2026-07-23",
                duplicate_id: true,
            },
        ),
        (
            ProviderId::Cninfo,
            AnnouncementFixtureProvider {
                record_provider: ProviderId::Cninfo,
                batch_source: "cninfo",
                instrument: requested,
                published_at: "2026-07-23",
                duplicate_id: false,
            },
        ),
    ];
    let mut router = AnnouncementRouter::new(AcceptancePolicy::new());
    for (provider_id, provider) in providers {
        router
            .register(announcement_source(
                provider_id,
                Arc::new(provider),
                classify,
            ))
            .unwrap();
    }

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Cninfo);
    assert_eq!(outcome.attempts().len(), 4);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[2].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Quality,
            ..
        }
    ));
}

struct StaticAnnouncementProvider {
    batch: DataBatch<Announcement>,
}

impl Announcements for StaticAnnouncementProvider {
    type Error = FixtureError;

    fn announcements(
        &self,
        _request: &InstrumentDateRangeRequest,
    ) -> Result<DataBatch<Announcement>, Self::Error> {
        Ok(self.batch.clone())
    }
}

fn static_announcement(id: &str, published_at: &str, source_at: Option<&str>) -> Announcement {
    let batch_id = "static-announcement";
    let mut evidence = SourceEvidence::new(ProviderId::Cninfo, "observed", batch_id).unwrap();
    if let Some(source_at) = source_at {
        evidence = evidence.with_source_at(source_at).unwrap();
    }
    Announcement {
        announcement_id: NonEmptyText::new(id).unwrap(),
        instrument: instrument(),
        instrument_name: None,
        category: None,
        title: NonEmptyText::new("fixture announcement").unwrap(),
        published_at: NonEmptyText::new(published_at).unwrap(),
        canonical_url: HttpsUrl::new("https://example.com/announcement/static").unwrap(),
        pdf_url: None,
        evidence,
    }
}

fn static_announcement_batch(records: Vec<Announcement>) -> DataBatch<Announcement> {
    DataBatch::strict(
        records,
        Provenance::new("cninfo", "observed")
            .unwrap()
            .with_batch_id("static-announcement")
            .unwrap(),
    )
}

#[test]
fn announcement_adapter_rejects_oversize_and_every_timestamp_evidence_shape() {
    let request = InstrumentDateRangeRequest::new(instrument(), PositiveU32::new(1).unwrap())
        .unwrap()
        .with_range(
            IsoDate::new("2026-07-01").unwrap(),
            IsoDate::new("2026-07-23").unwrap(),
        )
        .unwrap();
    let cases = [
        static_announcement_batch(vec![
            static_announcement("A001", "2026-07-23", Some("2026-07-23")),
            static_announcement("A002", "2026-07-23", Some("2026-07-23")),
        ]),
        static_announcement_batch(vec![static_announcement("A001", "2026-07-23", None)]),
        static_announcement_batch(vec![static_announcement(
            "A001",
            "2026-07-23",
            Some("2026-07-22"),
        )]),
        static_announcement_batch(vec![static_announcement("A001", "2026", Some("2026"))]),
        static_announcement_batch(vec![static_announcement(
            "A001",
            "2026-07-23X09:00:00",
            Some("2026-07-23X09:00:00"),
        )]),
        static_announcement_batch(vec![static_announcement(
            "A001",
            "2026-02-30",
            Some("2026-02-30"),
        )]),
    ];
    for batch in cases {
        let source = announcement_source(
            ProviderId::Cninfo,
            Arc::new(StaticAnnouncementProvider { batch }),
            classify,
        );
        assert!(source.fetch(&request).is_err());
    }
}

struct DragonTigerFixtureProvider {
    provider: ProviderId,
    source: &'static str,
    instrument: InstrumentId,
    trading_date: IsoDate,
    duplicate_entry: bool,
    complete_seats: bool,
}

impl DragonTigerFixtureProvider {
    fn evidence(&self, batch_id: &str) -> SourceEvidence {
        SourceEvidence::new(self.provider, "observed", batch_id)
            .unwrap()
            .with_source_at(self.trading_date.as_str())
            .unwrap()
    }

    fn provenance(&self, batch_id: &str) -> Provenance {
        Provenance::new(self.source, "observed")
            .unwrap()
            .with_source_at(self.trading_date.as_str())
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap()
    }
}

impl DragonTigerData for DragonTigerFixtureProvider {
    type Error = FixtureError;

    fn dragon_tiger_entries(
        &self,
        _request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerEntry>, Self::Error> {
        let batch_id = format!("{}-dragon-entry", self.source);
        let entry = DragonTigerEntry::new(
            NonEmptyText::new("entry-1").unwrap(),
            self.instrument.clone(),
            self.trading_date.clone(),
            Some(NonEmptyText::new("fixture").unwrap()),
            None,
            None,
            None,
            None,
            self.evidence(&batch_id),
        )
        .unwrap();
        let records = if self.duplicate_entry {
            vec![entry.clone(), entry]
        } else {
            vec![entry]
        };
        Ok(DataBatch::strict(records, self.provenance(&batch_id)))
    }

    fn dragon_tiger_seats(
        &self,
        _request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerSeat>, Self::Error> {
        let batch_id = format!("{}-dragon-seat", self.source);
        let count = if self.complete_seats { 10 } else { 9 };
        let records = (0..count)
            .map(|index| {
                let (side, rank) = if index < 5 {
                    (DragonTigerSide::Buy, index + 1)
                } else {
                    (DragonTigerSide::Sell, index - 4)
                };
                let amount = Money::new(1.0).unwrap();
                let (buy_amount, sell_amount) = match side {
                    DragonTigerSide::Buy => (Some(amount), None),
                    DragonTigerSide::Sell => (None, Some(amount)),
                };
                DragonTigerSeat::new(
                    NonEmptyText::new("entry-1").unwrap(),
                    self.instrument.clone(),
                    self.trading_date.clone(),
                    side,
                    PositiveU32::new(rank).unwrap(),
                    NonEmptyText::new(format!("seat-{index}")).unwrap(),
                    amount,
                    buy_amount,
                    sell_amount,
                    None,
                    self.evidence(&batch_id),
                )
                .unwrap()
            })
            .collect();
        Ok(DataBatch::strict(records, self.provenance(&batch_id)))
    }
}

#[test]
fn dragon_tiger_entry_adapter_rejects_wrong_identity_and_duplicate_ids() {
    let request = InstrumentSignalRequest::new(instrument(), PositiveU32::new(2).unwrap())
        .unwrap()
        .with_trading_date(IsoDate::new("2026-07-22").unwrap());
    let providers = [
        (
            ProviderId::Sse,
            DragonTigerFixtureProvider {
                provider: ProviderId::Sse,
                source: "sse",
                instrument: InstrumentId::new(Exchange::Shanghai, "600000", AssetClass::Equity)
                    .unwrap(),
                trading_date: IsoDate::new("2026-07-22").unwrap(),
                duplicate_entry: false,
                complete_seats: true,
            },
        ),
        (
            ProviderId::Szse,
            DragonTigerFixtureProvider {
                provider: ProviderId::Szse,
                source: "szse",
                instrument: instrument(),
                trading_date: IsoDate::new("2026-07-22").unwrap(),
                duplicate_entry: true,
                complete_seats: true,
            },
        ),
        (
            ProviderId::Eastmoney,
            DragonTigerFixtureProvider {
                provider: ProviderId::Eastmoney,
                source: "eastmoney",
                instrument: instrument(),
                trading_date: IsoDate::new("2026-07-22").unwrap(),
                duplicate_entry: false,
                complete_seats: true,
            },
        ),
    ];
    let mut router = DragonTigerEntryRouter::new(AcceptancePolicy::new());
    for (provider_id, provider) in providers {
        router
            .register(dragon_tiger_entry_source(
                provider_id,
                Arc::new(provider),
                classify,
            ))
            .unwrap();
    }
    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Quality,
            ..
        }
    ));
}

#[test]
fn dragon_tiger_seat_adapter_rejects_wrong_instrument_and_partial_top_five_group() {
    let request = InstrumentSignalRequest::new(instrument(), PositiveU32::new(20).unwrap())
        .unwrap()
        .with_trading_date(IsoDate::new("2026-07-22").unwrap());
    let wrong_instrument = Arc::new(DragonTigerFixtureProvider {
        provider: ProviderId::Sse,
        source: "sse",
        instrument: InstrumentId::new(Exchange::Shanghai, "600000", AssetClass::Equity).unwrap(),
        trading_date: IsoDate::new("2026-07-22").unwrap(),
        duplicate_entry: false,
        complete_seats: true,
    });
    let partial = Arc::new(DragonTigerFixtureProvider {
        provider: ProviderId::Szse,
        source: "szse",
        instrument: instrument(),
        trading_date: IsoDate::new("2026-07-22").unwrap(),
        duplicate_entry: false,
        complete_seats: false,
    });
    let valid = Arc::new(DragonTigerFixtureProvider {
        provider: ProviderId::Eastmoney,
        source: "eastmoney",
        instrument: instrument(),
        trading_date: IsoDate::new("2026-07-22").unwrap(),
        duplicate_entry: false,
        complete_seats: true,
    });
    let mut router = DragonTigerSeatRouter::new(AcceptancePolicy::new());
    router
        .register(dragon_tiger_seat_source(
            ProviderId::Sse,
            wrong_instrument,
            classify,
        ))
        .unwrap();
    router
        .register(dragon_tiger_seat_source(
            ProviderId::Szse,
            partial,
            classify,
        ))
        .unwrap();
    router
        .register(dragon_tiger_seat_source(
            ProviderId::Eastmoney,
            valid,
            classify,
        ))
        .unwrap();
    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert_eq!(outcome.batch().records().len(), 10);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Quality,
            ..
        }
    ));
}

struct MarketDragonTigerFixtureProvider {
    provider: ProviderId,
    source: &'static str,
    trading_date: IsoDate,
    descending: bool,
}

impl MarketDragonTigerFixtureProvider {
    fn disclosure(
        &self,
        batch_id: &str,
        entry_id: &str,
        instrument: InstrumentId,
        net_amount: f64,
    ) -> DragonTigerDisclosure {
        let evidence = SourceEvidence::new(self.provider, "observed", batch_id)
            .unwrap()
            .with_source_at(self.trading_date.as_str())
            .unwrap();
        let net_amount = Money::new(net_amount).unwrap();
        let entry = DragonTigerEntry::new(
            NonEmptyText::new(entry_id).unwrap(),
            instrument.clone(),
            self.trading_date.clone(),
            Some(NonEmptyText::new(format!("reason-{entry_id}")).unwrap()),
            Some(Money::new(net_amount.get() + 1.0).unwrap()),
            Some(Money::new(1.0).unwrap()),
            Some(net_amount),
            None,
            evidence.clone(),
        )
        .unwrap();
        let seats = (0..10)
            .map(|index| {
                let (side, rank) = if index < 5 {
                    (DragonTigerSide::Buy, index + 1)
                } else {
                    (DragonTigerSide::Sell, index - 4)
                };
                let amount = Money::new(1.0).unwrap();
                let (buy_amount, sell_amount) = match side {
                    DragonTigerSide::Buy => (Some(amount), None),
                    DragonTigerSide::Sell => (None, Some(amount)),
                };
                DragonTigerSeat::new(
                    NonEmptyText::new(entry_id).unwrap(),
                    instrument.clone(),
                    self.trading_date.clone(),
                    side,
                    PositiveU32::new(rank).unwrap(),
                    NonEmptyText::new(format!("seat-{entry_id}-{index}")).unwrap(),
                    amount,
                    buy_amount,
                    sell_amount,
                    None,
                    evidence.clone(),
                )
                .unwrap()
            })
            .collect();
        DragonTigerDisclosure::new(entry, seats).unwrap()
    }
}

impl MarketDragonTigerData for MarketDragonTigerFixtureProvider {
    type Error = FixtureError;

    fn market_dragon_tiger(
        &self,
        _request: &MarketDragonTigerRequest,
    ) -> Result<DataBatch<DragonTigerDisclosure>, Self::Error> {
        let batch_id = format!("{}-market-dragon-tiger", self.source);
        let high = self.disclosure(
            &batch_id,
            "entry-high",
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
            2.0,
        );
        let low = self.disclosure(
            &batch_id,
            "entry-low",
            InstrumentId::new(Exchange::Shenzhen, "002396", AssetClass::Equity).unwrap(),
            1.0,
        );
        let records = if self.descending {
            vec![high, low]
        } else {
            vec![low, high]
        };
        Ok(DataBatch::strict(
            records,
            Provenance::new(self.source, "observed")
                .unwrap()
                .with_source_at(self.trading_date.as_str())
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        ))
    }
}

#[test]
fn market_dragon_tiger_adapter_rejects_unsorted_disclosures() {
    let trading_date = IsoDate::new("2026-07-22").unwrap();
    let request =
        MarketDragonTigerRequest::new(trading_date.clone(), PositiveU32::new(2).unwrap()).unwrap();
    let mut router = MarketDragonTigerRouter::new(AcceptancePolicy::new());
    router
        .register(market_dragon_tiger_source(
            ProviderId::Sse,
            Arc::new(MarketDragonTigerFixtureProvider {
                provider: ProviderId::Sse,
                source: "sse",
                trading_date: trading_date.clone(),
                descending: false,
            }),
            classify,
        ))
        .unwrap();
    router
        .register(market_dragon_tiger_source(
            ProviderId::Eastmoney,
            Arc::new(MarketDragonTigerFixtureProvider {
                provider: ProviderId::Eastmoney,
                source: "eastmoney",
                trading_date,
                descending: true,
            }),
            classify,
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert_eq!(outcome.batch().records().len(), 2);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Quality,
            ..
        }
    ));
}

struct StaticDragonTigerProvider {
    entries: DataBatch<DragonTigerEntry>,
    seats: DataBatch<DragonTigerSeat>,
}

impl DragonTigerData for StaticDragonTigerProvider {
    type Error = FixtureError;

    fn dragon_tiger_entries(
        &self,
        _request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerEntry>, Self::Error> {
        Ok(self.entries.clone())
    }

    fn dragon_tiger_seats(
        &self,
        _request: &InstrumentSignalRequest,
    ) -> Result<DataBatch<DragonTigerSeat>, Self::Error> {
        Ok(self.seats.clone())
    }
}

fn static_provenance(batch_id: &str, source_at: Option<&str>) -> Provenance {
    let mut provenance = Provenance::new("eastmoney", "observed")
        .unwrap()
        .with_batch_id(batch_id)
        .unwrap();
    if let Some(source_at) = source_at {
        provenance = provenance.with_source_at(source_at).unwrap();
    }
    provenance
}

fn static_dragon_entry(
    entry_id: &str,
    trading_date: &str,
    evidence_date: &str,
    batch_id: &str,
) -> DragonTigerEntry {
    DragonTigerEntry::new(
        NonEmptyText::new(entry_id).unwrap(),
        instrument(),
        IsoDate::new(trading_date).unwrap(),
        None,
        None,
        None,
        None,
        None,
        SourceEvidence::new(ProviderId::Eastmoney, "observed", batch_id)
            .unwrap()
            .with_source_at(evidence_date)
            .unwrap(),
    )
    .unwrap()
}

fn static_dragon_seats(batch_id: &str, trading_date: &str) -> Vec<DragonTigerSeat> {
    (0..10)
        .map(|index| {
            let (side, rank) = if index < 5 {
                (DragonTigerSide::Buy, index + 1)
            } else {
                (DragonTigerSide::Sell, index - 4)
            };
            let amount = Money::new(1.0).unwrap();
            DragonTigerSeat::new(
                NonEmptyText::new("entry-1").unwrap(),
                instrument(),
                IsoDate::new(trading_date).unwrap(),
                side,
                PositiveU32::new(rank).unwrap(),
                NonEmptyText::new(format!("seat-{index}")).unwrap(),
                amount,
                (side == DragonTigerSide::Buy).then_some(amount),
                (side == DragonTigerSide::Sell).then_some(amount),
                None,
                SourceEvidence::new(ProviderId::Eastmoney, "observed", batch_id)
                    .unwrap()
                    .with_source_at(trading_date)
                    .unwrap(),
            )
            .unwrap()
        })
        .collect()
}

fn empty_entry_batch() -> DataBatch<DragonTigerEntry> {
    DataBatch::strict(
        Vec::new(),
        static_provenance("empty-entry", Some("2026-07-23")),
    )
}

fn empty_seat_batch() -> DataBatch<DragonTigerSeat> {
    DataBatch::strict(
        Vec::new(),
        static_provenance("empty-seat", Some("2026-07-23")),
    )
}

#[test]
fn dragon_tiger_adapters_reject_oversize_date_rank_duplicate_and_empty_groups() {
    let entry_request = InstrumentSignalRequest::new(instrument(), PositiveU32::new(1).unwrap())
        .unwrap()
        .with_trading_date(IsoDate::new("2026-07-23").unwrap());
    let entry_batch_id = "static-entry";
    let oversized_entries = DataBatch::strict(
        vec![
            static_dragon_entry("entry-1", "2026-07-23", "2026-07-23", entry_batch_id),
            static_dragon_entry("entry-2", "2026-07-23", "2026-07-23", entry_batch_id),
        ],
        static_provenance(entry_batch_id, Some("2026-07-23")),
    );
    let oversized = dragon_tiger_entry_source(
        ProviderId::Eastmoney,
        Arc::new(StaticDragonTigerProvider {
            entries: oversized_entries,
            seats: empty_seat_batch(),
        }),
        classify,
    );
    assert_eq!(
        oversized.fetch(&entry_request).unwrap_err().kind(),
        FailureKind::Quality
    );

    let wrong_record_date = DataBatch::strict(
        vec![static_dragon_entry(
            "entry-1",
            "2026-07-22",
            "2026-07-22",
            entry_batch_id,
        )],
        static_provenance(entry_batch_id, Some("2026-07-23")),
    );
    let wrong_record_date = dragon_tiger_entry_source(
        ProviderId::Eastmoney,
        Arc::new(StaticDragonTigerProvider {
            entries: wrong_record_date,
            seats: empty_seat_batch(),
        }),
        classify,
    );
    assert_eq!(
        wrong_record_date.fetch(&entry_request).unwrap_err().kind(),
        FailureKind::Evidence
    );

    let seat_request = InstrumentSignalRequest::new(instrument(), PositiveU32::new(20).unwrap())
        .unwrap()
        .with_trading_date(IsoDate::new("2026-07-23").unwrap());
    let seat_batch_id = "static-seat";
    let mut duplicate = static_dragon_seats(seat_batch_id, "2026-07-23");
    duplicate.push(duplicate[0].clone());
    let duplicate = dragon_tiger_seat_source(
        ProviderId::Eastmoney,
        Arc::new(StaticDragonTigerProvider {
            entries: empty_entry_batch(),
            seats: DataBatch::strict(
                duplicate,
                static_provenance(seat_batch_id, Some("2026-07-23")),
            ),
        }),
        classify,
    );
    assert_eq!(
        duplicate.fetch(&seat_request).unwrap_err().kind(),
        FailureKind::Quality
    );

    let empty = dragon_tiger_seat_source(
        ProviderId::Eastmoney,
        Arc::new(StaticDragonTigerProvider {
            entries: empty_entry_batch(),
            seats: empty_seat_batch(),
        }),
        classify,
    );
    assert_eq!(
        empty.fetch(&seat_request).unwrap_err().kind(),
        FailureKind::Quality
    );
}

#[test]
fn dragon_tiger_adapters_validate_optional_and_batch_dates_before_records() {
    let entry_batch_id = "date-entry";
    let entry = static_dragon_entry("entry-1", "2026-07-23", "2026-07-23", entry_batch_id);
    let request_without_date =
        InstrumentSignalRequest::new(instrument(), PositiveU32::new(1).unwrap()).unwrap();
    let undated_source = dragon_tiger_entry_source(
        ProviderId::Eastmoney,
        Arc::new(StaticDragonTigerProvider {
            entries: DataBatch::strict(
                vec![entry.clone()],
                static_provenance(entry_batch_id, None),
            ),
            seats: empty_seat_batch(),
        }),
        classify,
    );
    assert!(undated_source.fetch(&request_without_date).is_ok());

    let dated_request = request_without_date
        .clone()
        .with_trading_date(IsoDate::new("2026-07-23").unwrap());
    for source_at in [None, Some("2026-07-23X15:00:00")] {
        let source = dragon_tiger_entry_source(
            ProviderId::Eastmoney,
            Arc::new(StaticDragonTigerProvider {
                entries: DataBatch::strict(
                    vec![entry.clone()],
                    static_provenance(entry_batch_id, source_at),
                ),
                seats: empty_seat_batch(),
            }),
            classify,
        );
        assert_eq!(
            source.fetch(&dated_request).unwrap_err().kind(),
            FailureKind::Evidence
        );
    }

    let seat_batch_id = "date-seat";
    let seats = static_dragon_seats(seat_batch_id, "2026-07-23");
    let undated_seats = dragon_tiger_seat_source(
        ProviderId::Eastmoney,
        Arc::new(StaticDragonTigerProvider {
            entries: empty_entry_batch(),
            seats: DataBatch::strict(seats.clone(), static_provenance(seat_batch_id, None)),
        }),
        classify,
    );
    let seat_request_without_date =
        InstrumentSignalRequest::new(instrument(), PositiveU32::new(10).unwrap()).unwrap();
    assert!(undated_seats.fetch(&seat_request_without_date).is_ok());

    let oversized_seats = dragon_tiger_seat_source(
        ProviderId::Eastmoney,
        Arc::new(StaticDragonTigerProvider {
            entries: empty_entry_batch(),
            seats: DataBatch::strict(seats, static_provenance(seat_batch_id, Some("2026-07-23"))),
        }),
        classify,
    );
    let limit_nine =
        InstrumentSignalRequest::new(instrument(), PositiveU32::new(9).unwrap()).unwrap();
    assert_eq!(
        oversized_seats.fetch(&limit_nine).unwrap_err().kind(),
        FailureKind::Quality
    );

    let wrong_date_seats = dragon_tiger_seat_source(
        ProviderId::Eastmoney,
        Arc::new(StaticDragonTigerProvider {
            entries: empty_entry_batch(),
            seats: DataBatch::strict(
                static_dragon_seats(seat_batch_id, "2026-07-22"),
                static_provenance(seat_batch_id, Some("2026-07-23")),
            ),
        }),
        classify,
    );
    let dated_seat_request =
        seat_request_without_date.with_trading_date(IsoDate::new("2026-07-23").unwrap());
    assert_eq!(
        wrong_date_seats
            .fetch(&dated_seat_request)
            .unwrap_err()
            .kind(),
        FailureKind::Evidence
    );
}

struct StaticMarketDragonTigerProvider {
    batch: DataBatch<DragonTigerDisclosure>,
}

impl MarketDragonTigerData for StaticMarketDragonTigerProvider {
    type Error = FixtureError;

    fn market_dragon_tiger(
        &self,
        _request: &MarketDragonTigerRequest,
    ) -> Result<DataBatch<DragonTigerDisclosure>, Self::Error> {
        Ok(self.batch.clone())
    }
}

#[test]
fn market_dragon_tiger_adapter_rejects_empty_oversize_wrong_date_and_duplicates() {
    let request = MarketDragonTigerRequest::new(
        IsoDate::new("2026-07-23").unwrap(),
        PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let fixture = MarketDragonTigerFixtureProvider {
        provider: ProviderId::Eastmoney,
        source: "eastmoney",
        trading_date: IsoDate::new("2026-07-23").unwrap(),
        descending: true,
    };
    let batch_id = "static-market";
    let first = fixture.disclosure(batch_id, "entry-1", instrument(), 2.0);
    let second = fixture.disclosure(
        batch_id,
        "entry-2",
        InstrumentId::new(Exchange::Shenzhen, "002396", AssetClass::Equity).unwrap(),
        1.0,
    );
    let cases = [
        DataBatch::strict(Vec::new(), static_provenance(batch_id, Some("2026-07-23"))),
        DataBatch::strict(
            vec![first.clone(), second],
            static_provenance(batch_id, Some("2026-07-23")),
        ),
        DataBatch::strict(
            vec![first.clone(), first],
            static_provenance(batch_id, Some("2026-07-23")),
        ),
    ];
    for batch in cases {
        let source = market_dragon_tiger_source(
            ProviderId::Eastmoney,
            Arc::new(StaticMarketDragonTigerProvider { batch }),
            classify,
        );
        assert!(source.fetch(&request).is_err());
    }

    let wrong_date_fixture = MarketDragonTigerFixtureProvider {
        provider: ProviderId::Eastmoney,
        source: "eastmoney",
        trading_date: IsoDate::new("2026-07-22").unwrap(),
        descending: true,
    };
    let wrong_date = wrong_date_fixture.disclosure(batch_id, "entry-wrong-date", instrument(), 1.0);
    let source = market_dragon_tiger_source(
        ProviderId::Eastmoney,
        Arc::new(StaticMarketDragonTigerProvider {
            batch: DataBatch::strict(
                vec![wrong_date],
                static_provenance(batch_id, Some("2026-07-23")),
            ),
        }),
        classify,
    );
    assert_eq!(
        source.fetch(&request).unwrap_err().kind(),
        FailureKind::Evidence
    );
}

#[test]
fn market_dragon_tiger_adapter_checks_batch_dates_duplicates_and_exchange_order() {
    let date = IsoDate::new("2026-07-23").unwrap();
    let fixture = MarketDragonTigerFixtureProvider {
        provider: ProviderId::Eastmoney,
        source: "eastmoney",
        trading_date: date.clone(),
        descending: true,
    };
    let batch_id = "market-date";
    let first = fixture.disclosure(batch_id, "entry-1", instrument(), 2.0);
    let request =
        MarketDragonTigerRequest::new(date.clone(), PositiveU32::new(2).unwrap()).unwrap();

    for source_at in [None, Some("2026-07-23X15:00:00")] {
        let source = market_dragon_tiger_source(
            ProviderId::Eastmoney,
            Arc::new(StaticMarketDragonTigerProvider {
                batch: DataBatch::strict(
                    vec![first.clone()],
                    static_provenance(batch_id, source_at),
                ),
            }),
            classify,
        );
        assert_eq!(
            source.fetch(&request).unwrap_err().kind(),
            FailureKind::Evidence
        );
    }

    let duplicate_source = market_dragon_tiger_source(
        ProviderId::Eastmoney,
        Arc::new(StaticMarketDragonTigerProvider {
            batch: DataBatch::strict(
                vec![first.clone(), first],
                static_provenance(batch_id, Some("2026-07-23")),
            ),
        }),
        classify,
    );
    assert_eq!(
        duplicate_source.fetch(&request).unwrap_err().kind(),
        FailureKind::Quality
    );

    let equal_net = vec![
        fixture.disclosure(
            batch_id,
            "entry-sh",
            InstrumentId::new(Exchange::Shanghai, "600396", AssetClass::Equity).unwrap(),
            1.0,
        ),
        fixture.disclosure(
            batch_id,
            "entry-sz",
            InstrumentId::new(Exchange::Shenzhen, "002396", AssetClass::Equity).unwrap(),
            1.0,
        ),
        fixture.disclosure(
            batch_id,
            "entry-bj",
            InstrumentId::new(Exchange::Beijing, "920118", AssetClass::Equity).unwrap(),
            1.0,
        ),
    ];
    let ordered_source = market_dragon_tiger_source(
        ProviderId::Eastmoney,
        Arc::new(StaticMarketDragonTigerProvider {
            batch: DataBatch::strict(equal_net, static_provenance(batch_id, Some("2026-07-23"))),
        }),
        classify,
    );
    let order_request = MarketDragonTigerRequest::new(date, PositiveU32::new(3).unwrap()).unwrap();
    assert!(ordered_source.fetch(&order_request).is_ok());
}

struct PostCloseFixtureProvider {
    record_provider: ProviderId,
    batch_provider_name: &'static str,
    response_date: Option<&'static str>,
    duplicate_rank: bool,
    seen_dates: Mutex<Vec<String>>,
    seen_limits: Mutex<Vec<u32>>,
}

impl PostCloseFlows for PostCloseFixtureProvider {
    type Error = FixtureError;

    fn post_close_flows(
        &self,
        request: &PostCloseFlowRequest,
    ) -> Result<DataBatch<PostCloseFlow>, Self::Error> {
        self.seen_dates
            .lock()
            .unwrap()
            .push(request.trading_date().as_str().to_owned());
        self.seen_limits.lock().unwrap().push(request.limit().get());
        let response_date = self
            .response_date
            .unwrap_or_else(|| request.trading_date().as_str());
        let batch_id = format!("post-close-{response_date}");
        let source_at = format!("{response_date} 15:35:00");
        let record = PostCloseFlow::new(
            instrument(),
            Some(NonEmptyText::new("华电辽能").unwrap()),
            magic_market_core::IsoDate::new(response_date).unwrap(),
            magic_market_core::PositiveU32::new(1).unwrap(),
            Price::new(16.41).unwrap(),
            Ratio::new(9.99, RatioUnit::Percent).unwrap(),
            Money::new(100_000_000.0).unwrap(),
            Ratio::new(12.34, RatioUnit::Percent).unwrap(),
            None,
            None,
            SourceEvidence::new(
                self.record_provider,
                format!("{response_date}T15:35:00+08:00"),
                &batch_id,
            )
            .unwrap()
            .with_source_at(&source_at)
            .unwrap(),
        )
        .unwrap();
        let records = if self.duplicate_rank {
            vec![record.clone(), record]
        } else {
            vec![record]
        };
        Ok(DataBatch::strict(
            records,
            Provenance::new(self.batch_provider_name, "observed")
                .unwrap()
                .with_source_at(source_at)
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        ))
    }
}

#[test]
fn post_close_adapter_forwards_date_and_routes_only_matching_sourced_records() {
    let wrong = Arc::new(PostCloseFixtureProvider {
        record_provider: ProviderId::Eastmoney,
        batch_provider_name: "tencent",
        response_date: None,
        duplicate_rank: false,
        seen_dates: Mutex::new(Vec::new()),
        seen_limits: Mutex::new(Vec::new()),
    });
    let valid = Arc::new(PostCloseFixtureProvider {
        record_provider: ProviderId::Eastmoney,
        batch_provider_name: "eastmoney",
        response_date: None,
        duplicate_rank: false,
        seen_dates: Mutex::new(Vec::new()),
        seen_limits: Mutex::new(Vec::new()),
    });
    let request = PostCloseFlowRequest::new(
        magic_market_core::IsoDate::new("2026-07-23").unwrap(),
        magic_market_core::PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let mut router = PostCloseFlowRouter::new(
        AcceptancePolicy::new()
            .with_require_complete(true)
            .with_require_source_at(true),
    );
    router
        .register(post_close_flow_source(
            ProviderId::Tencent,
            Arc::clone(&wrong),
            classify,
        ))
        .unwrap();
    router
        .register(post_close_flow_source(
            ProviderId::Eastmoney,
            Arc::clone(&valid),
            classify,
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert_eq!(outcome.batch().records().len(), 1);
    assert_eq!(
        outcome.batch().records()[0].evidence().source_at(),
        Some("2026-07-23 15:35:00")
    );
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert_eq!(wrong.seen_dates.lock().unwrap().as_slice(), ["2026-07-23"]);
    assert_eq!(valid.seen_dates.lock().unwrap().as_slice(), ["2026-07-23"]);
    assert_eq!(wrong.seen_limits.lock().unwrap().as_slice(), [1]);
    assert_eq!(valid.seen_limits.lock().unwrap().as_slice(), [1]);
}

#[test]
fn post_close_adapter_rejects_wrong_dates_and_duplicate_ranks() {
    let wrong_date = Arc::new(PostCloseFixtureProvider {
        record_provider: ProviderId::Tencent,
        batch_provider_name: "tencent",
        response_date: Some("2026-07-22"),
        duplicate_rank: false,
        seen_dates: Mutex::new(Vec::new()),
        seen_limits: Mutex::new(Vec::new()),
    });
    let duplicate_rank = Arc::new(PostCloseFixtureProvider {
        record_provider: ProviderId::Sina,
        batch_provider_name: "sina",
        response_date: None,
        duplicate_rank: true,
        seen_dates: Mutex::new(Vec::new()),
        seen_limits: Mutex::new(Vec::new()),
    });
    let valid = Arc::new(PostCloseFixtureProvider {
        record_provider: ProviderId::Eastmoney,
        batch_provider_name: "eastmoney",
        response_date: None,
        duplicate_rank: false,
        seen_dates: Mutex::new(Vec::new()),
        seen_limits: Mutex::new(Vec::new()),
    });
    let request = PostCloseFlowRequest::new(
        magic_market_core::IsoDate::new("2026-07-23").unwrap(),
        magic_market_core::PositiveU32::new(1).unwrap(),
    )
    .unwrap();
    let mut router = PostCloseFlowRouter::new(AcceptancePolicy::new());
    router
        .register(post_close_flow_source(
            ProviderId::Tencent,
            wrong_date,
            classify,
        ))
        .unwrap();
    router
        .register(post_close_flow_source(
            ProviderId::Sina,
            duplicate_rank,
            classify,
        ))
        .unwrap();
    router
        .register(post_close_flow_source(
            ProviderId::Eastmoney,
            valid,
            classify,
        ))
        .unwrap();

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Eastmoney);
    assert_eq!(outcome.attempts().len(), 3);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Quality,
            ..
        }
    ));
}

#[derive(Clone, Copy)]
enum PostCloseFault {
    MissingBatchSourceTime,
    InvalidBatchSourceDateSuffix,
    ExceedsLimit,
    WrongRecordDate,
    DuplicateRank,
    NonContiguousRank,
    DuplicateInstrument,
    MissingName,
    NotDescending,
}

struct FaultyPostCloseProvider(PostCloseFault);

impl PostCloseFlows for FaultyPostCloseProvider {
    type Error = FixtureError;

    fn post_close_flows(
        &self,
        request: &PostCloseFlowRequest,
    ) -> Result<DataBatch<PostCloseFlow>, Self::Error> {
        let requested = request.trading_date().as_str();
        let record_date = if matches!(self.0, PostCloseFault::WrongRecordDate) {
            "2026-07-22"
        } else {
            requested
        };
        let source_at = format!("{requested} 15:35:00");
        let record_source_at = format!("{record_date} 15:35:00");
        let batch_id = format!("post-close-fault-{requested}");
        let make_record = |code: &str, rank: u32, name: Option<&str>, main_net: f64| {
            PostCloseFlow::new(
                InstrumentId::new(Exchange::Shanghai, code, AssetClass::Equity).unwrap(),
                name.map(|value| NonEmptyText::new(value).unwrap()),
                magic_market_core::IsoDate::new(record_date).unwrap(),
                magic_market_core::PositiveU32::new(rank).unwrap(),
                Price::new(16.41).unwrap(),
                Ratio::new(9.99, RatioUnit::Percent).unwrap(),
                Money::new(main_net).unwrap(),
                Ratio::new(12.5, RatioUnit::Percent).unwrap(),
                None,
                None,
                SourceEvidence::new(
                    ProviderId::Eastmoney,
                    format!("{record_date}T15:35:00+08:00"),
                    &batch_id,
                )
                .unwrap()
                .with_source_at(&record_source_at)
                .unwrap(),
            )
            .unwrap()
        };
        let records = match self.0 {
            PostCloseFault::ExceedsLimit => vec![
                make_record("600396", 1, Some("stock-1"), 100.0),
                make_record("600397", 2, Some("stock-2"), 90.0),
            ],
            PostCloseFault::DuplicateRank => vec![
                make_record("600396", 1, Some("stock-1"), 100.0),
                make_record("600397", 1, Some("stock-2"), 90.0),
            ],
            PostCloseFault::NonContiguousRank => vec![
                make_record("600396", 1, Some("stock-1"), 100.0),
                make_record("600397", 3, Some("stock-2"), 90.0),
            ],
            PostCloseFault::DuplicateInstrument => vec![
                make_record("600396", 1, Some("stock-1"), 100.0),
                make_record("600396", 2, Some("stock-2"), 90.0),
            ],
            PostCloseFault::MissingName => {
                vec![make_record("600396", 1, None, 100.0)]
            }
            PostCloseFault::NotDescending => vec![
                make_record("600396", 1, Some("stock-1"), 90.0),
                make_record("600397", 2, Some("stock-2"), 100.0),
            ],
            _ => vec![make_record("600396", 1, Some("stock-1"), 100.0)],
        };
        let mut provenance = Provenance::new("eastmoney", "observed")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap();
        if !matches!(self.0, PostCloseFault::MissingBatchSourceTime) {
            provenance = provenance
                .with_source_at(
                    if matches!(self.0, PostCloseFault::InvalidBatchSourceDateSuffix) {
                        format!("{requested}x15:35:00")
                    } else {
                        source_at
                    },
                )
                .unwrap();
        }
        Ok(DataBatch::strict(records, provenance))
    }
}

struct NorthboundFixtureProvider {
    record_channel: NorthboundChannel,
    batch_source_date: &'static str,
    omit_batch_source: bool,
    record_count: usize,
}

impl NorthboundDailyStatistics for NorthboundFixtureProvider {
    type Error = FixtureError;

    fn northbound_daily_statistics(
        &self,
        request: &NorthboundDailyRequest,
    ) -> Result<DataBatch<NorthboundDailyStat>, Self::Error> {
        let exchange = match self.record_channel {
            NorthboundChannel::Shanghai => Exchange::Shanghai,
            NorthboundChannel::Shenzhen => Exchange::Shenzhen,
        };
        let top_turnover = (1..=10)
            .map(|rank| {
                NorthboundTopTurnover::new(
                    PositiveU32::new(rank).unwrap(),
                    InstrumentId::new(
                        exchange,
                        format!("{:06}", 600_000 + rank),
                        AssetClass::Equity,
                    )
                    .unwrap(),
                    NonEmptyText::new(format!("stock-{rank}")).unwrap(),
                    Money::new(f64::from(rank)).unwrap(),
                )
                .unwrap()
            })
            .collect();
        let batch_id = format!("hkex-{}", request.trading_date().as_str());
        let record = NorthboundDailyStat::new(
            request.trading_date().clone(),
            self.record_channel,
            Money::new(100.0).unwrap(),
            Quantity::new(10.0).unwrap(),
            NorthboundQuotaBalance::Unavailable,
            Money::new(1.0).unwrap(),
            top_turnover,
            SourceEvidence::new(ProviderId::Hkex, "observed", &batch_id)
                .unwrap()
                .with_source_at(request.trading_date().as_str())
                .unwrap(),
        )
        .unwrap();
        let mut provenance = Provenance::new("hkex", "observed")
            .unwrap()
            .with_batch_id(batch_id)
            .unwrap();
        if !self.omit_batch_source {
            provenance = provenance.with_source_at(self.batch_source_date).unwrap();
        }
        Ok(DataBatch::strict(
            vec![record; self.record_count],
            provenance,
        ))
    }
}

#[test]
fn post_close_adapter_rejects_every_batch_and_record_contract_violation() {
    for (fault, limit, expected_kind) in [
        (
            PostCloseFault::MissingBatchSourceTime,
            1,
            FailureKind::Evidence,
        ),
        (
            PostCloseFault::InvalidBatchSourceDateSuffix,
            1,
            FailureKind::Evidence,
        ),
        (PostCloseFault::ExceedsLimit, 1, FailureKind::Quality),
        (PostCloseFault::WrongRecordDate, 1, FailureKind::Evidence),
        (PostCloseFault::DuplicateRank, 2, FailureKind::Quality),
        (PostCloseFault::NonContiguousRank, 2, FailureKind::Quality),
        (PostCloseFault::DuplicateInstrument, 2, FailureKind::Quality),
        (PostCloseFault::MissingName, 1, FailureKind::Quality),
        (PostCloseFault::NotDescending, 2, FailureKind::Quality),
    ] {
        let request = PostCloseFlowRequest::new(
            magic_market_core::IsoDate::new("2026-07-23").unwrap(),
            magic_market_core::PositiveU32::new(limit).unwrap(),
        )
        .unwrap();
        let source = post_close_flow_source(
            ProviderId::Eastmoney,
            Arc::new(FaultyPostCloseProvider(fault)),
            classify,
        );
        let error = source.fetch(&request).unwrap_err();
        assert_eq!(error.kind(), expected_kind);
    }
}

#[test]
fn northbound_adapter_rejects_wrong_channel_and_batch_date() {
    let wrong_channel = Arc::new(NorthboundFixtureProvider {
        record_channel: NorthboundChannel::Shenzhen,
        batch_source_date: "2026-07-22",
        omit_batch_source: false,
        record_count: 1,
    });
    let wrong_date = Arc::new(NorthboundFixtureProvider {
        record_channel: NorthboundChannel::Shanghai,
        batch_source_date: "2026-07-21",
        omit_batch_source: false,
        record_count: 1,
    });
    let valid = Arc::new(NorthboundFixtureProvider {
        record_channel: NorthboundChannel::Shanghai,
        batch_source_date: "2026-07-22",
        omit_batch_source: false,
        record_count: 1,
    });
    let request = NorthboundDailyRequest::new(
        IsoDate::new("2026-07-22").unwrap(),
        NorthboundChannel::Shanghai,
    );
    let mut router = NorthboundDailyRouter::new(
        AcceptancePolicy::new()
            .with_require_complete(true)
            .with_require_source_at(true),
    );
    for (provider_id, provider) in [
        (ProviderId::Sse, wrong_channel),
        (ProviderId::Szse, wrong_date),
        (ProviderId::Hkex, valid),
    ] {
        router
            .register(northbound_daily_source(provider_id, provider, classify))
            .unwrap();
    }

    let outcome = router.route(&request).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Hkex);
    assert_eq!(outcome.attempts().len(), 3);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Failed {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn northbound_adapter_rejects_missing_batch_time_and_wrong_cardinality() {
    let request = NorthboundDailyRequest::new(
        IsoDate::new("2026-07-22").unwrap(),
        NorthboundChannel::Shanghai,
    );
    for provider in [
        NorthboundFixtureProvider {
            record_channel: NorthboundChannel::Shanghai,
            batch_source_date: "2026-07-22",
            omit_batch_source: true,
            record_count: 1,
        },
        NorthboundFixtureProvider {
            record_channel: NorthboundChannel::Shanghai,
            batch_source_date: "2026-07-22",
            omit_batch_source: false,
            record_count: 2,
        },
    ] {
        let source = northbound_daily_source(ProviderId::Hkex, Arc::new(provider), classify);
        assert!(source.fetch(&request).is_err());
    }
}

#[test]
fn intelligence_adapters_reuse_evidence_preserving_failover() {
    let wrong = Arc::new(FixtureProvider {
        record_provider: ProviderId::Tencent,
        record_batch: "wrong",
        batch_provider_name: "eastmoney",
        batch_id: "wrong",
    });
    let valid = Arc::new(FixtureProvider {
        record_provider: ProviderId::Tencent,
        record_batch: "valid",
        batch_provider_name: "tencent",
        batch_id: "valid",
    });
    let mut router = MarketStatisticsRouter::new(AcceptancePolicy::new());
    router
        .register(market_statistics_source(
            ProviderId::Eastmoney,
            wrong,
            classify,
        ))
        .unwrap();
    router
        .register(market_statistics_source(
            ProviderId::Tencent,
            valid,
            classify,
        ))
        .unwrap();

    let outcome = router.route(&[instrument()]).unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Tencent);
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
    assert!(matches!(
        outcome.attempts()[1].status(),
        AttemptStatus::Selected
    ));
}

#[test]
fn dragon_tiger_adapters_accept_fully_dated_record_and_batch_evidence() {
    let date = "2026-07-23";
    let entry_batch_id = "dated-entry";
    let entry_source = dragon_tiger_entry_source(
        ProviderId::Eastmoney,
        Arc::new(StaticDragonTigerProvider {
            entries: DataBatch::strict(
                vec![static_dragon_entry("entry-1", date, date, entry_batch_id)],
                static_provenance(entry_batch_id, Some(date)),
            ),
            seats: empty_seat_batch(),
        }),
        classify,
    );
    let entry_request = InstrumentSignalRequest::new(instrument(), PositiveU32::new(1).unwrap())
        .unwrap()
        .with_trading_date(IsoDate::new(date).unwrap());
    assert_eq!(
        entry_source.fetch(&entry_request).unwrap().records().len(),
        1
    );

    let seat_batch_id = "dated-seats";
    let seat_source = dragon_tiger_seat_source(
        ProviderId::Eastmoney,
        Arc::new(StaticDragonTigerProvider {
            entries: empty_entry_batch(),
            seats: DataBatch::strict(
                static_dragon_seats(seat_batch_id, date),
                static_provenance(seat_batch_id, Some(date)),
            ),
        }),
        classify,
    );
    let seat_request = InstrumentSignalRequest::new(instrument(), PositiveU32::new(10).unwrap())
        .unwrap()
        .with_trading_date(IsoDate::new(date).unwrap());
    assert_eq!(
        seat_source.fetch(&seat_request).unwrap().records().len(),
        10
    );
}

fn disclosure_with_optional_net(
    batch_id: &str,
    entry_id: &str,
    instrument: InstrumentId,
    net_amount: Option<f64>,
) -> DragonTigerDisclosure {
    let date = IsoDate::new("2026-07-23").unwrap();
    let evidence = SourceEvidence::new(ProviderId::Eastmoney, "observed", batch_id)
        .unwrap()
        .with_source_at(date.as_str())
        .unwrap();
    let (buy_amount, sell_amount, net_amount) = match net_amount {
        Some(net) => (
            Some(Money::new(net + 1.0).unwrap()),
            Some(Money::new(1.0).unwrap()),
            Some(Money::new(net).unwrap()),
        ),
        None => (None, None, None),
    };
    let entry = DragonTigerEntry::new(
        NonEmptyText::new(entry_id).unwrap(),
        instrument.clone(),
        date.clone(),
        None,
        buy_amount,
        sell_amount,
        net_amount,
        None,
        evidence.clone(),
    )
    .unwrap();
    let seats = (0..10)
        .map(|index| {
            let (side, rank) = if index < 5 {
                (DragonTigerSide::Buy, index + 1)
            } else {
                (DragonTigerSide::Sell, index - 4)
            };
            let amount = Money::new(1.0).unwrap();
            DragonTigerSeat::new(
                NonEmptyText::new(entry_id).unwrap(),
                instrument.clone(),
                date.clone(),
                side,
                PositiveU32::new(rank).unwrap(),
                NonEmptyText::new(format!("seat-{entry_id}-{index}")).unwrap(),
                amount,
                (side == DragonTigerSide::Buy).then_some(amount),
                (side == DragonTigerSide::Sell).then_some(amount),
                None,
                evidence.clone(),
            )
            .unwrap()
        })
        .collect();
    DragonTigerDisclosure::new(entry, seats).unwrap()
}

fn static_market_source(
    batch_id: &str,
    records: Vec<DragonTigerDisclosure>,
) -> impl RoutedSource<MarketDragonTigerRequest, DragonTigerDisclosure> {
    market_dragon_tiger_source(
        ProviderId::Eastmoney,
        Arc::new(StaticMarketDragonTigerProvider {
            batch: DataBatch::strict(records, static_provenance(batch_id, Some("2026-07-23"))),
        }),
        classify,
    )
}

#[test]
fn market_dragon_tiger_order_covers_optional_net_and_tie_breakers() {
    let date = IsoDate::new("2026-07-23").unwrap();
    let request = |limit| {
        MarketDragonTigerRequest::new(date.clone(), PositiveU32::new(limit).unwrap()).unwrap()
    };

    let batch_id = "optional-net-ordered";
    let some = disclosure_with_optional_net(batch_id, "some", instrument(), Some(1.0));
    let none = disclosure_with_optional_net(
        batch_id,
        "none",
        InstrumentId::new(Exchange::Shenzhen, "002396", AssetClass::Equity).unwrap(),
        None,
    );
    assert!(
        static_market_source(batch_id, vec![some.clone(), none.clone()])
            .fetch(&request(2))
            .is_ok()
    );
    assert_eq!(
        static_market_source(batch_id, vec![none, some])
            .fetch(&request(2))
            .unwrap_err()
            .kind(),
        FailureKind::Quality
    );

    let batch_id = "optional-net-none";
    let sh_none = disclosure_with_optional_net(batch_id, "sh", instrument(), None);
    let sz_none = disclosure_with_optional_net(
        batch_id,
        "sz",
        InstrumentId::new(Exchange::Shenzhen, "002396", AssetClass::Equity).unwrap(),
        None,
    );
    assert!(static_market_source(batch_id, vec![sh_none, sz_none])
        .fetch(&request(2))
        .is_ok());

    let batch_id = "same-exchange-code-entry";
    let code_low = disclosure_with_optional_net(
        batch_id,
        "entry-0",
        InstrumentId::new(Exchange::Shanghai, "600001", AssetClass::Equity).unwrap(),
        Some(1.0),
    );
    let entry_low = disclosure_with_optional_net(
        batch_id,
        "entry-a",
        InstrumentId::new(Exchange::Shanghai, "600002", AssetClass::Equity).unwrap(),
        Some(1.0),
    );
    let entry_high = disclosure_with_optional_net(
        batch_id,
        "entry-b",
        InstrumentId::new(Exchange::Shanghai, "600002", AssetClass::Equity).unwrap(),
        Some(1.0),
    );
    assert!(
        static_market_source(batch_id, vec![code_low, entry_low, entry_high])
            .fetch(&request(3))
            .is_ok()
    );
}

#[test]
fn northbound_adapter_rejects_invalid_source_date_separator() {
    let request = NorthboundDailyRequest::new(
        IsoDate::new("2026-07-22").unwrap(),
        NorthboundChannel::Shanghai,
    );
    let source = northbound_daily_source(
        ProviderId::Hkex,
        Arc::new(NorthboundFixtureProvider {
            record_channel: NorthboundChannel::Shanghai,
            batch_source_date: "2026-07-22X15:00:00",
            omit_batch_source: false,
            record_count: 1,
        }),
        classify,
    );
    let error = source.fetch(&request).unwrap_err();
    assert_eq!(error.kind(), FailureKind::Evidence);
    assert!(error.message().contains("source date"));
}

struct FailingMarketDragonTigerProvider;

impl MarketDragonTigerData for FailingMarketDragonTigerProvider {
    type Error = FixtureError;

    fn market_dragon_tiger(
        &self,
        _request: &MarketDragonTigerRequest,
    ) -> Result<DataBatch<DragonTigerDisclosure>, Self::Error> {
        Err(FixtureError)
    }
}

struct FailingNorthboundProvider;

impl NorthboundDailyStatistics for FailingNorthboundProvider {
    type Error = FixtureError;

    fn northbound_daily_statistics(
        &self,
        _request: &NorthboundDailyRequest,
    ) -> Result<DataBatch<NorthboundDailyStat>, Self::Error> {
        Err(FixtureError)
    }
}

#[test]
fn market_dragon_tiger_and_northbound_preserve_error_classification() {
    let date = IsoDate::new("2026-07-23").unwrap();
    let market_request =
        MarketDragonTigerRequest::new(date.clone(), PositiveU32::new(1).unwrap()).unwrap();
    let market_source = market_dragon_tiger_source(
        ProviderId::Eastmoney,
        Arc::new(FailingMarketDragonTigerProvider),
        classify,
    );
    let error = market_source.fetch(&market_request).unwrap_err();
    assert_eq!(error.kind(), FailureKind::Transport);

    let northbound_request = NorthboundDailyRequest::new(date, NorthboundChannel::Shanghai);
    let northbound_source = northbound_daily_source(
        ProviderId::Hkex,
        Arc::new(FailingNorthboundProvider),
        classify,
    );
    let error = northbound_source.fetch(&northbound_request).unwrap_err();
    assert_eq!(error.kind(), FailureKind::Transport);
}

struct OptionFixtureProvider {
    record_provider: ProviderId,
    record_batch: &'static str,
    batch_provider_name: &'static str,
    batch_id: &'static str,
    seen_expiry: Mutex<Vec<Option<String>>>,
}

impl OptionFixtureProvider {
    fn new(
        record_provider: ProviderId,
        record_batch: &'static str,
        batch_provider_name: &'static str,
        batch_id: &'static str,
    ) -> Self {
        Self {
            record_provider,
            record_batch,
            batch_provider_name,
            batch_id,
            seen_expiry: Mutex::new(Vec::new()),
        }
    }

    fn evidence(&self) -> SourceEvidence {
        SourceEvidence::new(self.record_provider, "observed", self.record_batch).unwrap()
    }

    fn provenance(&self) -> Provenance {
        Provenance::new(self.batch_provider_name, "observed")
            .unwrap()
            .with_batch_id(self.batch_id)
            .unwrap()
    }
}

impl OptionData for OptionFixtureProvider {
    type Error = FixtureError;

    fn option_contracts(
        &self,
        underlying: &InstrumentId,
        expiry: Option<&ContractMonth>,
    ) -> Result<DataBatch<OptionContract>, Self::Error> {
        self.seen_expiry
            .lock()
            .unwrap()
            .push(expiry.map(|month| month.as_str().to_owned()));
        Ok(DataBatch::strict(
            vec![OptionContract {
                contract_code: NonEmptyText::new("10012127").unwrap(),
                underlying: underlying.clone(),
                expiry_month: expiry
                    .cloned()
                    .unwrap_or_else(|| ContractMonth::new("2026-08").unwrap()),
                expiry: None,
                kind: OptionKind::Call,
                strike: None,
                evidence: self.evidence(),
            }],
            self.provenance(),
        ))
    }

    fn option_quotes(
        &self,
        contracts: &[NonEmptyText],
    ) -> Result<DataBatch<OptionQuote>, Self::Error> {
        Ok(DataBatch::strict(
            vec![OptionQuote {
                contract_code: contracts[0].clone(),
                name: None,
                bid: None,
                bid_quantity: None,
                ask: None,
                ask_quantity: None,
                last: None,
                previous_close: None,
                open: None,
                high: None,
                low: None,
                upper_limit: None,
                lower_limit: None,
                strike: None,
                volume: None,
                open_interest: None,
                amount: None,
                change: None,
                amplitude: None,
                quote_at: None,
                evidence: self.evidence(),
            }],
            self.provenance(),
        ))
    }

    fn option_greeks(
        &self,
        contracts: &[NonEmptyText],
    ) -> Result<DataBatch<OptionGreeks>, Self::Error> {
        Ok(DataBatch::strict(
            vec![OptionGreeks {
                contract_code: contracts[0].clone(),
                name: None,
                volume: None,
                delta: Some(FiniteNumber::new(0.5).unwrap()),
                gamma: None,
                theta: None,
                vega: None,
                rho: None,
                implied_volatility: None,
                high: None,
                low: None,
                trade_code: None,
                strike: None,
                last: None,
                theoretical_price: None,
                evidence: self.evidence(),
            }],
            self.provenance(),
        ))
    }
}

fn option_underlying() -> InstrumentId {
    InstrumentId::new(Exchange::Shanghai, "510050", AssetClass::Fund).unwrap()
}

#[test]
fn option_contract_adapter_forwards_month_and_rejects_wrong_evidence() {
    let wrong = Arc::new(OptionFixtureProvider::new(
        ProviderId::Sina,
        "wrong",
        "tencent",
        "wrong",
    ));
    let valid = Arc::new(OptionFixtureProvider::new(
        ProviderId::Sina,
        "valid",
        "sina",
        "valid",
    ));
    let mut router = OptionContractRouter::new(AcceptancePolicy::new());
    router
        .register(option_contract_source(ProviderId::Tencent, wrong, classify))
        .unwrap();
    router
        .register(option_contract_source(
            ProviderId::Sina,
            valid.clone(),
            classify,
        ))
        .unwrap();

    let month = ContractMonth::new("2026-08").unwrap();
    let outcome = router
        .route(&(option_underlying(), Some(month.clone())))
        .unwrap();
    assert_eq!(outcome.selected_provider(), ProviderId::Sina);
    assert_eq!(outcome.batch().records()[0].expiry_month, month);
    assert_eq!(
        valid.seen_expiry.lock().unwrap().as_slice(),
        &[Some("2026-08".to_owned())]
    );
    assert!(matches!(
        outcome.attempts()[0].status(),
        AttemptStatus::Rejected {
            kind: FailureKind::Evidence,
            ..
        }
    ));
}

#[test]
fn option_quote_and_greek_adapters_preserve_valid_batches() {
    let provider = Arc::new(OptionFixtureProvider::new(
        ProviderId::Sina,
        "valid",
        "sina",
        "valid",
    ));
    let contracts = vec![NonEmptyText::new("10012127").unwrap()];

    let mut quotes = OptionQuoteRouter::new(AcceptancePolicy::new());
    quotes
        .register(option_quote_source(
            ProviderId::Sina,
            provider.clone(),
            classify,
        ))
        .unwrap();
    assert_eq!(
        quotes
            .route(contracts.as_slice())
            .unwrap()
            .batch()
            .records()[0]
            .contract_code,
        contracts[0]
    );

    let mut greeks = OptionGreeksRouter::new(AcceptancePolicy::new());
    greeks
        .register(option_greeks_source(ProviderId::Sina, provider, classify))
        .unwrap();
    assert_eq!(
        greeks
            .route(contracts.as_slice())
            .unwrap()
            .batch()
            .records()[0]
            .delta
            .unwrap()
            .get(),
        0.5
    );
}
