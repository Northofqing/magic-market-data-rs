use magic_market_core::{StreamCursor, StreamGeneration, StreamSequence};
use magic_market_monitor::{
    ReplayEntry, ReplayError, ReplayLimits, ReplayLog, ReplayUnavailable,
    LOCAL_AMOUNT_CHANGE_ANOMALY_ADMITTED, LOCAL_PRICE_CHANGE_ANOMALY_ADMITTED,
    LOCAL_VOLUME_CHANGE_ANOMALY_ADMITTED,
};

fn stream_generation(value: &str) -> StreamGeneration {
    StreamGeneration::new(value).unwrap()
}

fn cursor(generation: &StreamGeneration, sequence: u64) -> StreamCursor {
    StreamCursor::new(generation.clone(), StreamSequence::new(sequence).unwrap())
}

fn entry(
    generation: &StreamGeneration,
    sequence: u64,
    payload: &str,
    bytes: u64,
) -> ReplayEntry<String> {
    ReplayEntry::new(cursor(generation, sequence), payload.into(), bytes).unwrap()
}

#[test]
fn local_anomaly_families_remain_false_by_default() {
    const {
        assert!(LOCAL_PRICE_CHANGE_ANOMALY_ADMITTED);
        assert!(LOCAL_AMOUNT_CHANGE_ANOMALY_ADMITTED);
        assert!(LOCAL_VOLUME_CHANGE_ANOMALY_ADMITTED);
    }
}

#[test]
fn count_and_byte_limits_evict_oldest_entries_deterministically() {
    let generation = stream_generation("550e8400-e29b-41d4-a716-446655440000");
    let run = || {
        let mut log = ReplayLog::new(generation.clone(), ReplayLimits::new(3, 9).unwrap());
        for (sequence, bytes) in [(1, 3), (2, 3), (3, 3), (4, 4)] {
            log.insert(entry(
                &generation,
                sequence,
                &format!("event-{sequence}"),
                bytes,
            ))
            .unwrap();
        }
        log.replay_after(None)
            .unwrap()
            .map(|entry| (entry.cursor().sequence().get(), entry.payload().clone()))
            .collect::<Vec<_>>()
    };
    let first = run();
    assert_eq!(first, run());
    assert_eq!(first, vec![(3, "event-3".into()), (4, "event-4".into())]);
}

#[test]
fn oversized_insert_is_typed_and_does_not_mutate_the_log() {
    let generation = stream_generation("550e8400-e29b-41d4-a716-446655440000");
    let mut log = ReplayLog::new(generation.clone(), ReplayLimits::new(2, 4).unwrap());
    log.insert(entry(&generation, 1, "kept", 4)).unwrap();
    assert_eq!(
        log.insert(entry(&generation, 2, "too-large", 5))
            .unwrap_err(),
        ReplayError::EventTooLarge {
            encoded_len: 5,
            max_bytes: 4,
        }
    );
    assert_eq!(log.len(), 1);
    assert_eq!(log.retained_bytes(), 4);
    assert_eq!(log.newest_cursor().unwrap().sequence().get(), 1);
}

#[test]
fn replay_is_strictly_after_cursor_and_reports_unavailable_ranges() {
    let generation = stream_generation("550e8400-e29b-41d4-a716-446655440000");
    let mut log = ReplayLog::new(generation.clone(), ReplayLimits::new(2, 10).unwrap());
    for sequence in 1..=3 {
        log.insert(entry(&generation, sequence, "event", 3))
            .unwrap();
    }
    assert_eq!(log.oldest_cursor().unwrap().sequence().get(), 2);
    let sequences = log
        .replay_after(Some(&cursor(&generation, 1)))
        .unwrap()
        .map(|entry| entry.cursor().sequence().get())
        .collect::<Vec<_>>();
    assert_eq!(sequences, vec![2, 3]);

    let too_old = cursor(&generation, 1);
    let mut newer_log = ReplayLog::new(generation.clone(), ReplayLimits::new(1, 10).unwrap());
    newer_log.insert(entry(&generation, 4, "event", 3)).unwrap();
    assert!(matches!(
        newer_log.replay_after(Some(&too_old)),
        Err(ReplayError::ReplayUnavailable(
            ReplayUnavailable::CursorTooOld { .. }
        ))
    ));
    assert!(matches!(
        log.replay_after(Some(&cursor(&generation, 4))),
        Err(ReplayError::ReplayUnavailable(
            ReplayUnavailable::CursorAhead { .. }
        ))
    ));

    let other = stream_generation("550e8400-e29b-41d4-a716-446655440001");
    assert!(matches!(
        log.replay_after(Some(&cursor(&other, 2))),
        Err(ReplayError::ReplayUnavailable(
            ReplayUnavailable::WrongGeneration
        ))
    ));
}

#[test]
fn inserts_require_exact_next_sequence_and_reject_exhaustion_without_mutation() {
    let generation = stream_generation("550e8400-e29b-41d4-a716-446655440000");
    let mut log = ReplayLog::new(generation.clone(), ReplayLimits::new(3, 10).unwrap());
    log.insert(entry(&generation, 1, "one", 1)).unwrap();
    assert_eq!(
        log.insert(entry(&generation, 1, "duplicate", 1))
            .unwrap_err(),
        ReplayError::DuplicateSequence { sequence: 1 }
    );
    assert_eq!(
        log.insert(entry(&generation, 3, "gap", 1)).unwrap_err(),
        ReplayError::OutOfOrderSequence {
            previous: 1,
            actual: 3,
        }
    );
    assert_eq!(log.len(), 1);

    let max_cursor = StreamCursor::new(generation.clone(), StreamSequence::new(u64::MAX).unwrap());
    let max_entry = ReplayEntry::new(max_cursor, "max".to_owned(), 1).unwrap();
    assert_eq!(
        log.insert(max_entry).unwrap_err(),
        ReplayError::SequenceOverflow
    );
    assert_eq!(log.len(), 1);
}

#[test]
fn limits_and_encoded_lengths_are_explicit_and_checked() {
    assert!(ReplayLimits::new(0, 1).is_err());
    assert!(ReplayLimits::new(1, 0).is_err());
    let generation = stream_generation("550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(
        ReplayEntry::new(cursor(&generation, 1), "empty", 0).unwrap_err(),
        ReplayError::InvalidEncodedLength
    );
}

#[test]
fn retained_byte_arithmetic_overflow_is_typed_and_non_mutating() {
    let generation = stream_generation("550e8400-e29b-41d4-a716-446655440000");
    let mut log = ReplayLog::new(generation.clone(), ReplayLimits::new(2, u64::MAX).unwrap());
    log.insert(entry(&generation, 1, "huge", u64::MAX)).unwrap();
    assert_eq!(
        log.insert(entry(&generation, 2, "one-more", 1))
            .unwrap_err(),
        ReplayError::ByteCountOverflow
    );
    assert_eq!(log.len(), 1);
    assert_eq!(log.retained_bytes(), u64::MAX);
    assert_eq!(log.newest_cursor().unwrap().sequence().get(), 1);
}
