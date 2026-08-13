use crate::{
    AssetClass, CoreError, EvidenceTimestamp, Exchange, InstrumentId, NonEmptyText, PositiveU32,
    ProviderId, SourceEvidence,
};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::fmt;

/// Canonical UUID identifying one in-memory stream generation.
///
/// A generation changes whenever stream continuity can no longer be proved.
/// Generation creation belongs to the process boundary; Core only validates
/// the provider-neutral wire value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct StreamGeneration(String);

impl StreamGeneration {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        if !is_canonical_non_nil_uuid(&value) {
            return Err(CoreError::InvalidValue {
                field: "stream_generation",
                value,
                reason: "must be a canonical lowercase non-nil UUID",
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StreamGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for StreamGeneration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

fn is_canonical_non_nil_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(byte),
        })
        && bytes.iter().any(|byte| *byte != b'0' && *byte != b'-')
}

/// Positive, strictly increasing position inside one stream generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct StreamSequence(u64);

impl StreamSequence {
    pub fn new(value: u64) -> Result<Self, CoreError> {
        if value == 0 {
            return Err(CoreError::InvalidValue {
                field: "stream_sequence",
                value: value.to_string(),
                reason: "must be positive",
            });
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }

    pub fn checked_next(self) -> Result<Self, CoreError> {
        self.0
            .checked_add(1)
            .ok_or_else(|| CoreError::InvalidRequest("stream sequence exhausted".into()))
            .and_then(Self::new)
    }
}

impl<'de> Deserialize<'de> for StreamSequence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u64::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Exact replay/delivery position in one stream generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StreamCursor {
    generation: StreamGeneration,
    sequence: StreamSequence,
}

impl StreamCursor {
    pub fn new(generation: StreamGeneration, sequence: StreamSequence) -> Self {
        Self {
            generation,
            sequence,
        }
    }

    pub fn generation(&self) -> &StreamGeneration {
        &self.generation
    }

    pub fn sequence(&self) -> StreamSequence {
        self.sequence
    }
}

/// Clock basis selected by a versioned monitoring rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationTimeBasis {
    /// Provider-supplied source time. Its presence and validity must be proved.
    ProviderSourceTime,
    /// Local observation time, which cannot satisfy strict source freshness.
    LocalObservationTime,
}

/// What the current evidence proves about one stream's continuity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuityState {
    Continuous,
    Unknown,
    PossibleGap,
    KnownGap,
}

/// Exact endpoint evidence consumed from the authorized local terminal.
///
/// This endpoint contract deliberately does not fabricate provider source time.
/// [`AnomalyInputEvidence`] adds the exact instrument, ordered cursor range,
/// continuity and canonical rule-input commitment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalTerminalObservationEvidence {
    first: SourceEvidence,
    last: SourceEvidence,
}

impl LocalTerminalObservationEvidence {
    pub fn new(first: SourceEvidence, last: SourceEvidence) -> Result<Self, CoreError> {
        validate_provider(
            "local terminal first observation evidence",
            &first,
            ProviderId::LocalTerminal,
        )?;
        validate_provider(
            "local terminal last observation evidence",
            &last,
            ProviderId::LocalTerminal,
        )?;
        Ok(Self { first, last })
    }

    pub fn first(&self) -> &SourceEvidence {
        &self.first
    }

    pub fn last(&self) -> &SourceEvidence {
        &self.last
    }
}

impl<'de> Deserialize<'de> for LocalTerminalObservationEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            first: SourceEvidence,
            last: SourceEvidence,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.first, wire.last).map_err(de::Error::custom)
    }
}

/// Evidence boundary for one locally derived market event.
///
/// Input endpoints must remain `LocalTerminal`; the derived evidence must be
/// labelled `LocalAnalysis`. Construction and deserialization enforce both
/// identities so downstream data cannot impersonate either source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalAnalysisEventEvidence {
    input: LocalTerminalObservationEvidence,
    derived: SourceEvidence,
}

impl LocalAnalysisEventEvidence {
    pub fn new(
        input: LocalTerminalObservationEvidence,
        derived: SourceEvidence,
    ) -> Result<Self, CoreError> {
        validate_provider(
            "local analysis derived evidence",
            &derived,
            ProviderId::LocalAnalysis,
        )?;
        Ok(Self { input, derived })
    }

    pub fn input(&self) -> &LocalTerminalObservationEvidence {
        &self.input
    }

    pub fn derived(&self) -> &SourceEvidence {
        &self.derived
    }
}

impl<'de> Deserialize<'de> for LocalAnalysisEventEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            input: LocalTerminalObservationEvidence,
            derived: SourceEvidence,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.input, wire.derived).map_err(de::Error::custom)
    }
}

fn validate_provider(
    context: &str,
    evidence: &SourceEvidence,
    expected: ProviderId,
) -> Result<(), CoreError> {
    if evidence.provider() != expected {
        return Err(CoreError::InvalidRequest(format!(
            "{context} must use {expected:?}"
        )));
    }
    Ok(())
}

const EVENT_SCHEMA_VERSION: u16 = 1;

macro_rules! digest_wire_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            fn from_validated_hex(value: String) -> Result<Self, CoreError> {
                validate_sha256_hex($field, &value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Self::from_validated_hex(String::deserialize(deserializer)?)
                    .map_err(de::Error::custom)
            }
        }
    };
}

digest_wire_type!(AnomalyRuleDigest, "anomaly_rule_digest");
digest_wire_type!(RuleInputDigest, "rule_input_digest");
digest_wire_type!(AnomalyInputDigest, "anomaly_input_digest");
digest_wire_type!(MarketEventId, "market_event_id");

impl AnomalyRuleDigest {
    /// Digests one complete rule definition using ordered, length-prefixed
    /// fields and a rule-definition-specific domain tag.
    ///
    /// Field names must be unique, lowercase ASCII identifiers in strictly
    /// increasing lexical order. Values are exact bytes: callers must encode
    /// numbers, units and optionality explicitly rather than relying on JSON
    /// object order or display formatting.
    pub fn from_canonical_fields(fields: &[(&str, &[u8])]) -> Result<Self, CoreError> {
        canonical_fields_digest(
            "magic-market/anomaly-rule-definition/v1",
            "anomaly rule definition",
            fields,
        )
        .and_then(Self::from_validated_hex)
    }
}

impl RuleInputDigest {
    /// Digests the complete rule-specific inputs using the same canonical field
    /// contract as [`AnomalyRuleDigest::from_canonical_fields`].
    ///
    /// Endpoint evidence, cursor range, selected time basis and both continuity
    /// states are added by [`AnomalyInputEvidence`]; this digest commits the
    /// remaining rule-specific values without freezing a price/amount/volume
    /// payload in Core.
    pub fn from_canonical_fields(fields: &[(&str, &[u8])]) -> Result<Self, CoreError> {
        canonical_fields_digest(
            "magic-market/anomaly-rule-inputs/v1",
            "anomaly rule inputs",
            fields,
        )
        .and_then(Self::from_validated_hex)
    }
}

/// Immutable identity of one versioned anomaly rule.
///
/// `definition_digest` commits thresholds, units, window, hysteresis, cooldown,
/// session policy and accepted continuity. Changing any of those facts requires
/// a new revision and digest; Core deliberately does not define unadmitted rule
/// bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnomalyRuleIdentity {
    id: NonEmptyText,
    revision: PositiveU32,
    definition_digest: AnomalyRuleDigest,
}

impl AnomalyRuleIdentity {
    pub fn new(
        id: impl Into<String>,
        revision: u32,
        definition_digest: AnomalyRuleDigest,
    ) -> Result<Self, CoreError> {
        let id = validated_rule_id(id.into())?;
        Ok(Self {
            id,
            revision: PositiveU32::new(revision)?,
            definition_digest,
        })
    }

    pub fn from_canonical_definition(
        id: impl Into<String>,
        revision: u32,
        fields: &[(&str, &[u8])],
    ) -> Result<Self, CoreError> {
        Self::new(
            id,
            revision,
            AnomalyRuleDigest::from_canonical_fields(fields)?,
        )
    }

    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    pub fn revision(&self) -> u32 {
        self.revision.get()
    }

    pub fn definition_digest(&self) -> &AnomalyRuleDigest {
        &self.definition_digest
    }
}

impl<'de> Deserialize<'de> for AnomalyRuleIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            id: String,
            revision: u32,
            definition_digest: AnomalyRuleDigest,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.id, wire.revision, wire.definition_digest).map_err(de::Error::custom)
    }
}

/// Public state transition emitted by an anomaly rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyTransition {
    Triggered,
    Escalated,
    Rearmed,
}

/// Independent local-observation-transport and provider-source continuity
/// claims.
///
/// A continuous polling/IPC observation sequence does not prove that the
/// upstream source emitted every market update, so these states can never be
/// collapsed into one flag. This contract is transport-neutral and does not
/// require a DLL, callback, subprocess or HTTP implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamContinuity {
    observation: ContinuityState,
    source: ContinuityState,
}

impl StreamContinuity {
    pub fn new(observation: ContinuityState, source: ContinuityState) -> Self {
        Self {
            observation,
            source,
        }
    }

    pub fn observation(self) -> ContinuityState {
        self.observation
    }

    pub fn source(self) -> ContinuityState {
        self.source
    }
}

/// Complete identity evidence consumed by a provider-neutral anomaly rule.
///
/// The endpoint records prove the authorized `LocalTerminal` source boundary.
/// The cursor range is same-generation and ordered. `rule_inputs_digest`
/// commits the complete rule-specific input bytes. `canonical_digest` is then
/// recomputed from all of those facts, the selected time basis, and both
/// continuity states during construction and deserialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnomalyInputEvidence {
    instrument: InstrumentId,
    terminal: LocalTerminalObservationEvidence,
    first_cursor: StreamCursor,
    last_cursor: StreamCursor,
    time_basis: ObservationTimeBasis,
    continuity: StreamContinuity,
    rule_inputs_digest: RuleInputDigest,
    canonical_digest: AnomalyInputDigest,
}

impl AnomalyInputEvidence {
    pub fn new(
        instrument: InstrumentId,
        terminal: LocalTerminalObservationEvidence,
        first_cursor: StreamCursor,
        last_cursor: StreamCursor,
        time_basis: ObservationTimeBasis,
        continuity: StreamContinuity,
        rule_inputs_digest: RuleInputDigest,
    ) -> Result<Self, CoreError> {
        validate_input_range(&terminal, &first_cursor, &last_cursor, time_basis)?;
        let canonical_digest = compute_anomaly_input_digest(
            &instrument,
            &terminal,
            &first_cursor,
            &last_cursor,
            time_basis,
            continuity,
            &rule_inputs_digest,
        )?;
        Ok(Self {
            instrument,
            terminal,
            first_cursor,
            last_cursor,
            time_basis,
            continuity,
            rule_inputs_digest,
            canonical_digest,
        })
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn terminal(&self) -> &LocalTerminalObservationEvidence {
        &self.terminal
    }

    pub fn first_cursor(&self) -> &StreamCursor {
        &self.first_cursor
    }

    pub fn last_cursor(&self) -> &StreamCursor {
        &self.last_cursor
    }

    pub fn time_basis(&self) -> ObservationTimeBasis {
        self.time_basis
    }

    pub fn continuity(&self) -> StreamContinuity {
        self.continuity
    }

    pub fn rule_inputs_digest(&self) -> &RuleInputDigest {
        &self.rule_inputs_digest
    }

    pub fn canonical_digest(&self) -> &AnomalyInputDigest {
        &self.canonical_digest
    }
}

impl<'de> Deserialize<'de> for AnomalyInputEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            instrument: InstrumentId,
            terminal: LocalTerminalObservationEvidence,
            first_cursor: StreamCursor,
            last_cursor: StreamCursor,
            time_basis: ObservationTimeBasis,
            continuity: StreamContinuity,
            rule_inputs_digest: RuleInputDigest,
            canonical_digest: AnomalyInputDigest,
        }

        let wire = Wire::deserialize(deserializer)?;
        let validated = Self::new(
            wire.instrument,
            wire.terminal,
            wire.first_cursor,
            wire.last_cursor,
            wire.time_basis,
            wire.continuity,
            wire.rule_inputs_digest,
        )
        .map_err(de::Error::custom)?;
        if validated.canonical_digest != wire.canonical_digest {
            return Err(de::Error::custom(
                "anomaly input canonical digest does not match its evidence",
            ));
        }
        Ok(validated)
    }
}

/// Provider-neutral anomaly envelope with a replay-stable deterministic ID.
///
/// Rule-specific trigger values remain behind `rule_inputs_digest` until their
/// individual data families are admitted. The derived observation timestamp is
/// retained as evidence but excluded from `event_id`; replay or delivery time
/// therefore cannot change identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnomalyEvent {
    schema_version: u16,
    event_id: MarketEventId,
    instrument: InstrumentId,
    rule: AnomalyRuleIdentity,
    transition: AnomalyTransition,
    stream: StreamCursor,
    input_evidence: AnomalyInputEvidence,
    derived_evidence: SourceEvidence,
}

impl AnomalyEvent {
    pub fn new(
        instrument: InstrumentId,
        rule: AnomalyRuleIdentity,
        transition: AnomalyTransition,
        stream: StreamCursor,
        input_evidence: AnomalyInputEvidence,
        derived_observed_at: impl Into<String>,
    ) -> Result<Self, CoreError> {
        validate_event_identity(&instrument, &stream, &input_evidence)?;
        let derived_observed_at = derived_observed_at.into();
        validate_derived_observation_time(&derived_observed_at, &input_evidence)?;
        let event_id = compute_anomaly_event_id(
            &instrument,
            &rule,
            transition,
            &stream,
            input_evidence.canonical_digest(),
        )?;
        let derived_evidence = SourceEvidence::new(
            ProviderId::LocalAnalysis,
            derived_observed_at,
            event_id.as_str(),
        )?;
        Ok(Self {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id,
            instrument,
            rule,
            transition,
            stream,
            input_evidence,
            derived_evidence,
        })
    }

    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn event_id(&self) -> &MarketEventId {
        &self.event_id
    }

    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    pub fn rule(&self) -> &AnomalyRuleIdentity {
        &self.rule
    }

    pub fn transition(&self) -> AnomalyTransition {
        self.transition
    }

    pub fn stream(&self) -> &StreamCursor {
        &self.stream
    }

    pub fn input_evidence(&self) -> &AnomalyInputEvidence {
        &self.input_evidence
    }

    pub fn derived_evidence(&self) -> &SourceEvidence {
        &self.derived_evidence
    }
}

impl<'de> Deserialize<'de> for AnomalyEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u16,
            event_id: MarketEventId,
            instrument: InstrumentId,
            rule: AnomalyRuleIdentity,
            transition: AnomalyTransition,
            stream: StreamCursor,
            input_evidence: AnomalyInputEvidence,
            derived_evidence: SourceEvidence,
        }

        let wire = Wire::deserialize(deserializer)?;
        if wire.schema_version != EVENT_SCHEMA_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported market event schema version {}",
                wire.schema_version
            )));
        }
        validate_event_identity(&wire.instrument, &wire.stream, &wire.input_evidence)
            .map_err(de::Error::custom)?;
        validate_provider(
            "anomaly derived evidence",
            &wire.derived_evidence,
            ProviderId::LocalAnalysis,
        )
        .map_err(de::Error::custom)?;
        if wire.derived_evidence.source_at().is_some() {
            return Err(de::Error::custom(
                "LocalAnalysis derived evidence must not fabricate provider source time",
            ));
        }
        validate_derived_observation_time(
            wire.derived_evidence.observed_at(),
            &wire.input_evidence,
        )
        .map_err(de::Error::custom)?;
        let expected = compute_anomaly_event_id(
            &wire.instrument,
            &wire.rule,
            wire.transition,
            &wire.stream,
            wire.input_evidence.canonical_digest(),
        )
        .map_err(de::Error::custom)?;
        if wire.event_id != expected {
            return Err(de::Error::custom(
                "anomaly event ID does not match its canonical identity",
            ));
        }
        if wire.derived_evidence.batch_id() != wire.event_id.as_str() {
            return Err(de::Error::custom(
                "anomaly derived batch ID must equal the canonical event ID",
            ));
        }
        Ok(Self {
            schema_version: wire.schema_version,
            event_id: wire.event_id,
            instrument: wire.instrument,
            rule: wire.rule,
            transition: wire.transition,
            stream: wire.stream,
            input_evidence: wire.input_evidence,
            derived_evidence: wire.derived_evidence,
        })
    }
}

/// Source/control status kinds that do not depend on an admitted market-data
/// field family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatusKind {
    Disconnected,
    Recovered,
    PossibleDataGap,
    KnownDataGap,
    ContinuityReset,
}

/// Ordered LocalTerminal source/control event.
///
/// `observed_at` is local status-observation time, never provider source time.
/// A recovery must name the different prior generation. Other statuses do not
/// carry one, preventing an ambiguous optional-generation contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceStatusEvent {
    schema_version: u16,
    event_id: MarketEventId,
    source_provider: ProviderId,
    status: SourceStatusKind,
    instrument: Option<InstrumentId>,
    stream: StreamCursor,
    previous_generation: Option<StreamGeneration>,
    observed_at: NonEmptyText,
    continuity: StreamContinuity,
}

impl SourceStatusEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        status: SourceStatusKind,
        instrument: Option<InstrumentId>,
        stream: StreamCursor,
        previous_generation: Option<StreamGeneration>,
        observed_at: impl Into<String>,
        continuity: StreamContinuity,
    ) -> Result<Self, CoreError> {
        validate_source_status(status, &stream, previous_generation.as_ref(), continuity)?;
        let observed_at = NonEmptyText::new(observed_at)?;
        EvidenceTimestamp::parse_instant(observed_at.as_str())?;
        let event_id = compute_source_status_event_id(
            ProviderId::LocalTerminal,
            status,
            instrument.as_ref(),
            &stream,
            previous_generation.as_ref(),
            observed_at.as_str(),
            continuity,
        )?;
        Ok(Self {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id,
            source_provider: ProviderId::LocalTerminal,
            status,
            instrument,
            stream,
            previous_generation,
            observed_at,
            continuity,
        })
    }

    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn event_id(&self) -> &MarketEventId {
        &self.event_id
    }

    pub fn source_provider(&self) -> ProviderId {
        self.source_provider
    }

    pub fn status(&self) -> SourceStatusKind {
        self.status
    }

    pub fn instrument(&self) -> Option<&InstrumentId> {
        self.instrument.as_ref()
    }

    pub fn stream(&self) -> &StreamCursor {
        &self.stream
    }

    pub fn previous_generation(&self) -> Option<&StreamGeneration> {
        self.previous_generation.as_ref()
    }

    pub fn observed_at(&self) -> &str {
        self.observed_at.as_str()
    }

    pub fn continuity(&self) -> StreamContinuity {
        self.continuity
    }
}

impl<'de> Deserialize<'de> for SourceStatusEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u16,
            event_id: MarketEventId,
            source_provider: ProviderId,
            status: SourceStatusKind,
            instrument: Option<InstrumentId>,
            stream: StreamCursor,
            previous_generation: Option<StreamGeneration>,
            observed_at: String,
            continuity: StreamContinuity,
        }

        let wire = Wire::deserialize(deserializer)?;
        if wire.schema_version != EVENT_SCHEMA_VERSION {
            return Err(de::Error::custom(format!(
                "unsupported market event schema version {}",
                wire.schema_version
            )));
        }
        if wire.source_provider != ProviderId::LocalTerminal {
            return Err(de::Error::custom(
                "local source status must use LocalTerminal",
            ));
        }
        let validated = Self::new(
            wire.status,
            wire.instrument,
            wire.stream,
            wire.previous_generation,
            wire.observed_at,
            wire.continuity,
        )
        .map_err(de::Error::custom)?;
        if validated.event_id != wire.event_id {
            return Err(de::Error::custom(
                "source status event ID does not match its canonical identity",
            ));
        }
        Ok(validated)
    }
}

/// Globally sequenced public market-event union.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MarketEvent {
    MarketAnomaly(Box<AnomalyEvent>),
    SourceStatus(SourceStatusEvent),
}

impl MarketEvent {
    pub fn event_id(&self) -> &MarketEventId {
        match self {
            Self::MarketAnomaly(event) => event.event_id(),
            Self::SourceStatus(event) => event.event_id(),
        }
    }

    pub fn stream(&self) -> &StreamCursor {
        match self {
            Self::MarketAnomaly(event) => event.stream(),
            Self::SourceStatus(event) => event.stream(),
        }
    }
}

fn validated_rule_id(value: String) -> Result<NonEmptyText, CoreError> {
    let id = NonEmptyText::new(value)?;
    let mut chars = id.as_str().chars();
    let first = chars.next().ok_or_else(|| CoreError::InvalidValue {
        field: "anomaly_rule_id",
        value: id.as_str().to_owned(),
        reason: "must not be empty",
    })?;
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(CoreError::InvalidValue {
            field: "anomaly_rule_id",
            value: id.as_str().to_owned(),
            reason: "must start with a lowercase ASCII letter or digit",
        });
    }
    if chars.any(|character| {
        !character.is_ascii_lowercase()
            && !character.is_ascii_digit()
            && !matches!(character, '-' | '_' | '.')
    }) {
        return Err(CoreError::InvalidValue {
            field: "anomaly_rule_id",
            value: id.as_str().to_owned(),
            reason: "must contain only lowercase ASCII letters, digits, '-', '_' or '.'",
        });
    }
    Ok(id)
}

fn validate_input_range(
    terminal: &LocalTerminalObservationEvidence,
    first_cursor: &StreamCursor,
    last_cursor: &StreamCursor,
    time_basis: ObservationTimeBasis,
) -> Result<(), CoreError> {
    let first_observed = EvidenceTimestamp::parse_instant(terminal.first().observed_at())?;
    let last_observed = EvidenceTimestamp::parse_instant(terminal.last().observed_at())?;
    if last_observed.duration_since(first_observed).is_none() {
        return Err(CoreError::InvalidRequest(
            "anomaly input observation times must be non-decreasing".into(),
        ));
    }
    if first_cursor.generation() != last_cursor.generation() {
        return Err(CoreError::InvalidRequest(
            "anomaly input cursor range must use one stream generation".into(),
        ));
    }
    if first_cursor.sequence() > last_cursor.sequence() {
        return Err(CoreError::InvalidRequest(
            "anomaly input cursor range must be non-decreasing".into(),
        ));
    }
    if time_basis == ObservationTimeBasis::ProviderSourceTime
        && (terminal.first().source_at().is_none() || terminal.last().source_at().is_none())
    {
        return Err(CoreError::InvalidRequest(
            "provider-source-time input requires source time on both endpoint observations".into(),
        ));
    }
    if time_basis == ObservationTimeBasis::ProviderSourceTime {
        let first_source = EvidenceTimestamp::parse_instant(
            terminal
                .first()
                .source_at()
                .expect("presence checked above"),
        )?;
        let last_source = EvidenceTimestamp::parse_instant(
            terminal.last().source_at().expect("presence checked above"),
        )?;
        if last_source.duration_since(first_source).is_none() {
            return Err(CoreError::InvalidRequest(
                "anomaly input provider source times must be non-decreasing".into(),
            ));
        }
    }
    Ok(())
}

fn validate_derived_observation_time(
    derived_observed_at: &str,
    input: &AnomalyInputEvidence,
) -> Result<(), CoreError> {
    let derived = EvidenceTimestamp::parse_instant(derived_observed_at)?;
    let last_input = EvidenceTimestamp::parse_instant(input.terminal().last().observed_at())?;
    if derived.duration_since(last_input).is_none() {
        return Err(CoreError::InvalidRequest(
            "anomaly derivation observation time must not precede its last input".into(),
        ));
    }
    Ok(())
}

fn validate_event_generation(
    stream: &StreamCursor,
    input: &AnomalyInputEvidence,
) -> Result<(), CoreError> {
    if stream.generation() != input.last_cursor().generation() {
        return Err(CoreError::InvalidRequest(
            "anomaly event and input range must use the same stream generation".into(),
        ));
    }
    Ok(())
}

fn validate_event_identity(
    instrument: &InstrumentId,
    stream: &StreamCursor,
    input: &AnomalyInputEvidence,
) -> Result<(), CoreError> {
    validate_event_generation(stream, input)?;
    if instrument != input.instrument() {
        return Err(CoreError::InvalidRequest(
            "anomaly event instrument must equal its input evidence instrument".into(),
        ));
    }
    Ok(())
}

fn validate_source_status(
    status: SourceStatusKind,
    stream: &StreamCursor,
    previous_generation: Option<&StreamGeneration>,
    continuity: StreamContinuity,
) -> Result<(), CoreError> {
    if status == SourceStatusKind::Recovered {
        let previous = previous_generation.ok_or_else(|| {
            CoreError::InvalidRequest(
                "source recovery must identify the previous stream generation".into(),
            )
        })?;
        if previous == stream.generation() {
            return Err(CoreError::InvalidRequest(
                "source recovery must move to a different stream generation".into(),
            ));
        }
        if !has_gap(continuity) {
            return Err(CoreError::InvalidRequest(
                "source recovery must report a possible or known gap".into(),
            ));
        }
    } else if previous_generation.is_some() {
        return Err(CoreError::InvalidRequest(
            "only source recovery may carry a previous stream generation".into(),
        ));
    }

    match status {
        SourceStatusKind::PossibleDataGap if !has_gap(continuity) => {
            Err(CoreError::InvalidRequest(
                "possible-gap status requires possible or known gap continuity".into(),
            ))
        }
        SourceStatusKind::KnownDataGap
            if continuity.observation() != ContinuityState::KnownGap
                && continuity.source() != ContinuityState::KnownGap =>
        {
            Err(CoreError::InvalidRequest(
                "known-gap status requires known-gap continuity".into(),
            ))
        }
        SourceStatusKind::Disconnected | SourceStatusKind::ContinuityReset
            if continuity.observation() == ContinuityState::Continuous
                && continuity.source() == ContinuityState::Continuous =>
        {
            Err(CoreError::InvalidRequest(
                "disconnect/reset status cannot report fully continuous input".into(),
            ))
        }
        _ => Ok(()),
    }
}

fn has_gap(continuity: StreamContinuity) -> bool {
    matches!(
        continuity.observation(),
        ContinuityState::PossibleGap | ContinuityState::KnownGap
    ) || matches!(
        continuity.source(),
        ContinuityState::PossibleGap | ContinuityState::KnownGap
    )
}

fn compute_anomaly_input_digest(
    instrument: &InstrumentId,
    terminal: &LocalTerminalObservationEvidence,
    first_cursor: &StreamCursor,
    last_cursor: &StreamCursor,
    time_basis: ObservationTimeBasis,
    continuity: StreamContinuity,
    rule_inputs_digest: &RuleInputDigest,
) -> Result<AnomalyInputDigest, CoreError> {
    let first_sequence = first_cursor.sequence().get().to_be_bytes();
    let last_sequence = last_cursor.sequence().get().to_be_bytes();
    let first_source_at = encode_optional_text(terminal.first().source_at());
    let last_source_at = encode_optional_text(terminal.last().source_at());
    let mut hasher = CanonicalHasher::new("magic-market/anomaly-input-evidence/v1")?;
    hasher.field(
        "first_cursor.generation",
        first_cursor.generation().as_str().as_bytes(),
    )?;
    hasher.field("first_cursor.sequence", &first_sequence)?;
    hasher.field(
        "first_evidence.batch_id",
        terminal.first().batch_id().as_bytes(),
    )?;
    hasher.field(
        "first_evidence.observed_at",
        terminal.first().observed_at().as_bytes(),
    )?;
    hasher.field("first_evidence.provider", b"LocalTerminal")?;
    hasher.field("first_evidence.source_at", &first_source_at)?;
    hasher.field(
        "instrument.asset_class",
        asset_class_name(instrument.asset_class()).as_bytes(),
    )?;
    hasher.field("instrument.code", instrument.code().as_bytes())?;
    hasher.field(
        "instrument.exchange",
        exchange_name(instrument.exchange()).as_bytes(),
    )?;
    hasher.field(
        "last_cursor.generation",
        last_cursor.generation().as_str().as_bytes(),
    )?;
    hasher.field("last_cursor.sequence", &last_sequence)?;
    hasher.field(
        "last_evidence.batch_id",
        terminal.last().batch_id().as_bytes(),
    )?;
    hasher.field(
        "last_evidence.observed_at",
        terminal.last().observed_at().as_bytes(),
    )?;
    hasher.field("last_evidence.provider", b"LocalTerminal")?;
    hasher.field("last_evidence.source_at", &last_source_at)?;
    hasher.field(
        "observation_continuity",
        continuity_name(continuity.observation()).as_bytes(),
    )?;
    hasher.field("rule_inputs_digest", rule_inputs_digest.as_str().as_bytes())?;
    hasher.field(
        "source_continuity",
        continuity_name(continuity.source()).as_bytes(),
    )?;
    hasher.field("time_basis", time_basis_name(time_basis).as_bytes())?;
    AnomalyInputDigest::from_validated_hex(hasher.finish()?)
}

fn compute_anomaly_event_id(
    instrument: &InstrumentId,
    rule: &AnomalyRuleIdentity,
    transition: AnomalyTransition,
    stream: &StreamCursor,
    input_digest: &AnomalyInputDigest,
) -> Result<MarketEventId, CoreError> {
    let revision = rule.revision().to_be_bytes();
    let sequence = stream.sequence().get().to_be_bytes();
    let mut hasher = CanonicalHasher::new("magic-market/anomaly-event/v1")?;
    hasher.field("input_digest", input_digest.as_str().as_bytes())?;
    hasher.field(
        "instrument.asset_class",
        asset_class_name(instrument.asset_class()).as_bytes(),
    )?;
    hasher.field("instrument.code", instrument.code().as_bytes())?;
    hasher.field(
        "instrument.exchange",
        exchange_name(instrument.exchange()).as_bytes(),
    )?;
    hasher.field(
        "rule.definition_digest",
        rule.definition_digest().as_str().as_bytes(),
    )?;
    hasher.field("rule.id", rule.id().as_bytes())?;
    hasher.field("rule.revision", &revision)?;
    hasher.field("stream.generation", stream.generation().as_str().as_bytes())?;
    hasher.field("stream.sequence", &sequence)?;
    hasher.field("transition", transition_name(transition).as_bytes())?;
    MarketEventId::from_validated_hex(hasher.finish()?)
}

#[allow(clippy::too_many_arguments)]
fn compute_source_status_event_id(
    source_provider: ProviderId,
    status: SourceStatusKind,
    instrument: Option<&InstrumentId>,
    stream: &StreamCursor,
    previous_generation: Option<&StreamGeneration>,
    observed_at: &str,
    continuity: StreamContinuity,
) -> Result<MarketEventId, CoreError> {
    let instrument_asset_class =
        encode_optional_text(instrument.map(|value| asset_class_name(value.asset_class())));
    let instrument_code = encode_optional_text(instrument.map(InstrumentId::code));
    let instrument_exchange =
        encode_optional_text(instrument.map(|value| exchange_name(value.exchange())));
    let previous_generation =
        encode_optional_text(previous_generation.map(StreamGeneration::as_str));
    let sequence = stream.sequence().get().to_be_bytes();
    let mut hasher = CanonicalHasher::new("magic-market/source-status-event/v1")?;
    hasher.field("instrument.asset_class", &instrument_asset_class)?;
    hasher.field("instrument.code", &instrument_code)?;
    hasher.field("instrument.exchange", &instrument_exchange)?;
    hasher.field(
        "observation_continuity",
        continuity_name(continuity.observation()).as_bytes(),
    )?;
    hasher.field("observed_at", observed_at.as_bytes())?;
    hasher.field("previous_generation", &previous_generation)?;
    hasher.field(
        "source_continuity",
        continuity_name(continuity.source()).as_bytes(),
    )?;
    hasher.field("source_provider", provider_name(source_provider).as_bytes())?;
    hasher.field("status", source_status_name(status).as_bytes())?;
    hasher.field("stream.generation", stream.generation().as_str().as_bytes())?;
    hasher.field("stream.sequence", &sequence)?;
    MarketEventId::from_validated_hex(hasher.finish()?)
}

fn canonical_fields_digest(
    domain: &str,
    context: &str,
    fields: &[(&str, &[u8])],
) -> Result<String, CoreError> {
    if fields.is_empty() {
        return Err(CoreError::InvalidRequest(format!(
            "{context} must contain at least one canonical field"
        )));
    }
    let mut hasher = CanonicalHasher::new(domain)?;
    for (name, value) in fields {
        validate_canonical_field_name(name)?;
        hasher.field(name, value)?;
    }
    hasher.finish()
}

fn validate_canonical_field_name(name: &str) -> Result<(), CoreError> {
    let mut characters = name.chars();
    let valid_first = characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit());
    let valid_rest = characters.all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '-' | '_' | '.')
    });
    if !valid_first || !valid_rest {
        return Err(CoreError::InvalidValue {
            field: "canonical_field_name",
            value: name.to_owned(),
            reason: "must be a lowercase ASCII identifier",
        });
    }
    Ok(())
}

fn validate_sha256_hex(field: &'static str, value: &str) -> Result<(), CoreError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CoreError::InvalidValue {
            field,
            value: value.to_owned(),
            reason: "must be exactly 64 lowercase hexadecimal SHA-256 characters",
        });
    }
    Ok(())
}

fn encode_optional_text(value: Option<&str>) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(value.map_or(1, |text| text.len().saturating_add(1)));
    match value {
        Some(text) => {
            encoded.push(1);
            encoded.extend_from_slice(text.as_bytes());
        }
        None => encoded.push(0),
    }
    encoded
}

fn exchange_name(value: Exchange) -> &'static str {
    match value {
        Exchange::Shanghai => "Shanghai",
        Exchange::Shenzhen => "Shenzhen",
        Exchange::Beijing => "Beijing",
    }
}

fn asset_class_name(value: AssetClass) -> &'static str {
    match value {
        AssetClass::Equity => "Equity",
        AssetClass::Index => "Index",
        AssetClass::Fund => "Fund",
        AssetClass::Bond => "Bond",
        AssetClass::Option => "Option",
    }
}

fn time_basis_name(value: ObservationTimeBasis) -> &'static str {
    match value {
        ObservationTimeBasis::ProviderSourceTime => "provider_source_time",
        ObservationTimeBasis::LocalObservationTime => "local_observation_time",
    }
}

fn continuity_name(value: ContinuityState) -> &'static str {
    match value {
        ContinuityState::Continuous => "continuous",
        ContinuityState::Unknown => "unknown",
        ContinuityState::PossibleGap => "possible_gap",
        ContinuityState::KnownGap => "known_gap",
    }
}

fn transition_name(value: AnomalyTransition) -> &'static str {
    match value {
        AnomalyTransition::Triggered => "triggered",
        AnomalyTransition::Escalated => "escalated",
        AnomalyTransition::Rearmed => "rearmed",
    }
}

fn source_status_name(value: SourceStatusKind) -> &'static str {
    match value {
        SourceStatusKind::Disconnected => "disconnected",
        SourceStatusKind::Recovered => "recovered",
        SourceStatusKind::PossibleDataGap => "possible_data_gap",
        SourceStatusKind::KnownDataGap => "known_data_gap",
        SourceStatusKind::ContinuityReset => "continuity_reset",
    }
}

fn provider_name(value: ProviderId) -> &'static str {
    match value {
        ProviderId::LocalTerminal => "LocalTerminal",
        _ => "unsupported",
    }
}

struct CanonicalHasher {
    sha256: Sha256,
    previous_field: Option<String>,
    field_count: u64,
}

impl CanonicalHasher {
    fn new(domain: &str) -> Result<Self, CoreError> {
        let mut sha256 = Sha256::new();
        sha256.update_length_prefixed(domain.as_bytes())?;
        Ok(Self {
            sha256,
            previous_field: None,
            field_count: 0,
        })
    }

    fn field(&mut self, name: &str, value: &[u8]) -> Result<(), CoreError> {
        if self
            .previous_field
            .as_deref()
            .is_some_and(|previous| previous >= name)
        {
            return Err(CoreError::InvalidRequest(format!(
                "canonical field names must be unique and strictly increasing: {name}"
            )));
        }
        self.sha256.update_length_prefixed(name.as_bytes())?;
        self.sha256.update_length_prefixed(value)?;
        self.field_count = self
            .field_count
            .checked_add(1)
            .ok_or_else(|| CoreError::InvalidRequest("canonical field count exhausted".into()))?;
        self.previous_field = Some(name.to_owned());
        Ok(())
    }

    fn finish(mut self) -> Result<String, CoreError> {
        let field_count = self.field_count.to_be_bytes();
        // A fixed terminator keeps the field count outside the last value's
        // length-prefixed boundary.
        self.sha256.update(&[0xff])?;
        self.sha256.update(&field_count)?;
        Ok(self.sha256.finish_hex())
    }
}

struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total_len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0; 64],
            buffer_len: 0,
            total_len: 0,
        }
    }

    fn update_length_prefixed(&mut self, bytes: &[u8]) -> Result<(), CoreError> {
        let length = u64::try_from(bytes.len())
            .map_err(|_| CoreError::InvalidRequest("canonical field length exceeds u64".into()))?;
        self.update(&length.to_be_bytes())?;
        self.update(bytes)
    }

    fn update(&mut self, bytes: &[u8]) -> Result<(), CoreError> {
        let length = u64::try_from(bytes.len())
            .map_err(|_| CoreError::InvalidRequest("SHA-256 input length exceeds u64".into()))?;
        self.total_len = self
            .total_len
            .checked_add(length)
            .filter(|total| *total <= u64::MAX / 8)
            .ok_or_else(|| CoreError::InvalidRequest("SHA-256 input length exhausted".into()))?;
        self.update_bytes(bytes);
        Ok(())
    }

    fn update_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.buffer[self.buffer_len] = *byte;
            self.buffer_len += 1;
            if self.buffer_len == self.buffer.len() {
                let block = self.buffer;
                self.compress(&block);
                self.buffer_len = 0;
            }
        }
    }

    fn finish_hex(mut self) -> String {
        let bit_length = self.total_len * 8;
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;
        if self.buffer_len > 56 {
            self.buffer[self.buffer_len..].fill(0);
            let block = self.buffer;
            self.compress(&block);
            self.buffer_len = 0;
        }
        self.buffer[self.buffer_len..56].fill(0);
        self.buffer[56..].copy_from_slice(&bit_length.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(64);
        for byte in self.state.into_iter().flat_map(u32::to_be_bytes) {
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        output
    }

    fn compress(&mut self, block: &[u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut schedule = [0_u32; 64];
        for (index, word) in block.chunks_exact(4).take(16).enumerate() {
            schedule[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let sigma0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let sigma1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(sigma0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(sigma1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for (word, constant) in schedule.into_iter().zip(K) {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(sum1)
                .wrapping_add(choice)
                .wrapping_add(constant)
                .wrapping_add(word);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}
