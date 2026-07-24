use magic_market_core::{
    Announcement, Announcements, AssetClass, ContractMonth, DataBatch, DragonTigerData,
    DragonTigerEntry, DragonTigerSeat, DragonTigerSide, Exchange, FiniteNumber, HttpsUrl,
    InstrumentDateRangeRequest, InstrumentId, InstrumentSignalRequest, IsoDate, MarketStatistics,
    MarketStatisticsProvider, Money, NewsItem, NewsProvider, NonEmptyText, NorthboundChannel,
    NorthboundDailyRequest, NorthboundDailyStat, NorthboundDailyStatistics,
    NorthboundQuotaBalance, NorthboundTopTurnover, OptionContract, OptionData, OptionGreeks,
    OptionKind, OptionQuote, PositiveU32, PostCloseFlow, PostCloseFlowRequest, PostCloseFlows,
    Price, Provenance, ProviderId, Quantity, Ratio, RatioUnit, SourceEvidence,
};
use magic_market_router::{
    announcement_source, dragon_tiger_entry_source, dragon_tiger_seat_source,
    global_news_source, market_statistics_source, northbound_daily_source, option_contract_source,
    option_greeks_source, option_quote_source, post_close_flow_source, AcceptancePolicy,
    AnnouncementRouter, AttemptStatus, DragonTigerEntryRouter, DragonTigerSeatRouter, FailureKind,
    GlobalNewsRouter, MarketStatisticsRouter, NorthboundDailyRouter, OptionContractRouter,
    OptionGreeksRouter, OptionQuoteRouter, PostCloseFlowRouter, SourceError,
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
        let evidence =
            SourceEvidence::new(self.record_provider, "observed", &batch_id)
                .unwrap()
                .with_source_at(published_at)
                .unwrap();
        Ok(DataBatch::strict(
            vec![NewsItem {
                item_id: NonEmptyText::new("news-1").unwrap(),
                title: NonEmptyText::new("fixture financial news").unwrap(),
                summary: None,
                content: None,
                publisher: NonEmptyText::new(self.batch_source).unwrap(),
                canonical_url: HttpsUrl::new("https://example.com/news/news-1").unwrap(),
                published_at: NonEmptyText::new(published_at).unwrap(),
                instruments: Vec::new(),
                topics: Vec::new(),
                language: NonEmptyText::new("zh-CN").unwrap(),
                evidence,
            }],
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
    });
    let valid = Arc::new(NewsFixtureProvider {
        record_provider: ProviderId::ThePaper,
        batch_source: "thepaper-finance-v1",
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
            None,
            None,
            SourceEvidence::new(self.record_provider, "observed", &batch_id)
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
        magic_market_core::PositiveU32::new(10).unwrap(),
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
    assert_eq!(wrong.seen_limits.lock().unwrap().as_slice(), [10]);
    assert_eq!(valid.seen_limits.lock().unwrap().as_slice(), [10]);
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
        magic_market_core::PositiveU32::new(10).unwrap(),
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

struct NorthboundFixtureProvider {
    record_channel: NorthboundChannel,
    batch_source_date: &'static str,
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
        Ok(DataBatch::strict(
            vec![record],
            Provenance::new("hkex", "observed")
                .unwrap()
                .with_source_at(self.batch_source_date)
                .unwrap()
                .with_batch_id(batch_id)
                .unwrap(),
        ))
    }
}

#[test]
fn northbound_adapter_rejects_wrong_channel_and_batch_date() {
    let wrong_channel = Arc::new(NorthboundFixtureProvider {
        record_channel: NorthboundChannel::Shenzhen,
        batch_source_date: "2026-07-22",
    });
    let wrong_date = Arc::new(NorthboundFixtureProvider {
        record_channel: NorthboundChannel::Shanghai,
        batch_source_date: "2026-07-21",
    });
    let valid = Arc::new(NorthboundFixtureProvider {
        record_channel: NorthboundChannel::Shanghai,
        batch_source_date: "2026-07-22",
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
