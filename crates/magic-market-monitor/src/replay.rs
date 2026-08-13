use magic_market_core::{StreamCursor, StreamGeneration, StreamSequence};
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

/// Explicit count and encoded-byte replay bounds. No production defaults exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayLimits {
    max_events: u32,
    max_bytes: u64,
}

impl ReplayLimits {
    pub fn new(max_events: u32, max_bytes: u64) -> Result<Self, ReplayError> {
        if max_events == 0 {
            return Err(ReplayError::InvalidConfiguration(
                "maximum replay event count must be positive",
            ));
        }
        if max_bytes == 0 {
            return Err(ReplayError::InvalidConfiguration(
                "maximum replay bytes must be positive",
            ));
        }
        Ok(Self {
            max_events,
            max_bytes,
        })
    }

    pub fn max_events(self) -> u32 {
        self.max_events
    }

    pub fn max_bytes(self) -> u64 {
        self.max_bytes
    }
}

/// One retained opaque event plus its exact externally computed encoded size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayEntry<T> {
    cursor: StreamCursor,
    payload: T,
    encoded_len: u64,
}

impl<T> ReplayEntry<T> {
    pub fn new(cursor: StreamCursor, payload: T, encoded_len: u64) -> Result<Self, ReplayError> {
        if encoded_len == 0 {
            return Err(ReplayError::InvalidEncodedLength);
        }
        Ok(Self {
            cursor,
            payload,
            encoded_len,
        })
    }

    pub fn cursor(&self) -> &StreamCursor {
        &self.cursor
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }

    pub fn encoded_len(&self) -> u64 {
        self.encoded_len
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    InvalidConfiguration(&'static str),
    InvalidEncodedLength,
    EventTooLarge { encoded_len: u64, max_bytes: u64 },
    WrongGeneration,
    DuplicateSequence { sequence: u64 },
    OutOfOrderSequence { previous: u64, actual: u64 },
    SequenceOverflow,
    ByteCountOverflow,
    ReplayUnavailable(ReplayUnavailable),
}

impl fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(formatter, "invalid replay configuration: {message}")
            }
            Self::InvalidEncodedLength => {
                formatter.write_str("replay encoded length must be positive")
            }
            Self::EventTooLarge {
                encoded_len,
                max_bytes,
            } => write!(
                formatter,
                "replay event size {encoded_len} exceeds byte limit {max_bytes}"
            ),
            Self::WrongGeneration => {
                formatter.write_str("replay insert uses a different stream generation")
            }
            Self::DuplicateSequence { sequence } => {
                write!(formatter, "duplicate replay sequence {sequence}")
            }
            Self::OutOfOrderSequence { previous, actual } => write!(
                formatter,
                "out-of-order replay sequence {actual} follows {previous}"
            ),
            Self::SequenceOverflow => formatter.write_str("replay sequence exhausted"),
            Self::ByteCountOverflow => formatter.write_str("replay byte count overflowed"),
            Self::ReplayUnavailable(reason) => write!(formatter, "replay unavailable: {reason}"),
        }
    }
}

impl Error for ReplayError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayUnavailable {
    WrongGeneration,
    CursorTooOld {
        requested: StreamCursor,
        oldest_available: StreamCursor,
    },
    CursorAhead {
        requested: StreamCursor,
        newest_available: StreamCursor,
    },
}

impl fmt::Display for ReplayUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongGeneration => formatter.write_str("cursor generation does not match"),
            Self::CursorTooOld { .. } => {
                formatter.write_str("cursor precedes the oldest retained sequence")
            }
            Self::CursorAhead { .. } => {
                formatter.write_str("cursor follows the newest retained sequence")
            }
        }
    }
}

/// Count-and-byte bounded, same-generation in-memory replay log.
pub struct ReplayLog<T> {
    generation: StreamGeneration,
    limits: ReplayLimits,
    entries: VecDeque<ReplayEntry<T>>,
    retained_bytes: u64,
    last_inserted_sequence: Option<StreamSequence>,
}

impl<T> ReplayLog<T> {
    pub fn new(generation: StreamGeneration, limits: ReplayLimits) -> Self {
        Self {
            generation,
            limits,
            entries: VecDeque::new(),
            retained_bytes: 0,
            last_inserted_sequence: None,
        }
    }

    pub fn generation(&self) -> &StreamGeneration {
        &self.generation
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }

    pub fn oldest_cursor(&self) -> Option<&StreamCursor> {
        self.entries.front().map(ReplayEntry::cursor)
    }

    pub fn newest_cursor(&self) -> Option<&StreamCursor> {
        self.entries.back().map(ReplayEntry::cursor)
    }

    pub fn insert(&mut self, entry: ReplayEntry<T>) -> Result<(), ReplayError> {
        if entry.cursor.generation() != &self.generation {
            return Err(ReplayError::WrongGeneration);
        }
        if entry.encoded_len > self.limits.max_bytes {
            return Err(ReplayError::EventTooLarge {
                encoded_len: entry.encoded_len,
                max_bytes: self.limits.max_bytes,
            });
        }
        if entry.cursor.sequence().get() == u64::MAX {
            return Err(ReplayError::SequenceOverflow);
        }
        if let Some(previous) = self.last_inserted_sequence {
            let actual = entry.cursor.sequence();
            if actual == previous {
                return Err(ReplayError::DuplicateSequence {
                    sequence: actual.get(),
                });
            }
            if actual < previous {
                return Err(ReplayError::OutOfOrderSequence {
                    previous: previous.get(),
                    actual: actual.get(),
                });
            }
            if previous
                .checked_next()
                .map_err(|_| ReplayError::SequenceOverflow)?
                != actual
            {
                return Err(ReplayError::OutOfOrderSequence {
                    previous: previous.get(),
                    actual: actual.get(),
                });
            }
        }

        let new_bytes = self
            .retained_bytes
            .checked_add(entry.encoded_len)
            .ok_or(ReplayError::ByteCountOverflow)?;
        self.retained_bytes = new_bytes;
        self.last_inserted_sequence = Some(entry.cursor.sequence());
        self.entries.push_back(entry);
        while self.entries.len() > self.limits.max_events as usize
            || self.retained_bytes > self.limits.max_bytes
        {
            let removed = self
                .entries
                .pop_front()
                .ok_or(ReplayError::ByteCountOverflow)?;
            self.retained_bytes = self
                .retained_bytes
                .checked_sub(removed.encoded_len)
                .ok_or(ReplayError::ByteCountOverflow)?;
        }
        Ok(())
    }

    /// Returns retained events strictly after the acknowledged cursor.
    ///
    /// `None` represents a new subscriber asking for every retained event.
    pub fn replay_after(
        &self,
        cursor: Option<&StreamCursor>,
    ) -> Result<impl Iterator<Item = &ReplayEntry<T>>, ReplayError> {
        let start = match cursor {
            None => 0,
            Some(cursor) => self.replay_start(cursor)?,
        };
        Ok(self.entries.iter().skip(start))
    }

    fn replay_start(&self, cursor: &StreamCursor) -> Result<usize, ReplayError> {
        if cursor.generation() != &self.generation {
            return Err(ReplayError::ReplayUnavailable(
                ReplayUnavailable::WrongGeneration,
            ));
        }
        let Some(oldest) = self.entries.front() else {
            return Ok(0);
        };
        let newest = self.entries.back().ok_or(ReplayError::ByteCountOverflow)?;
        let requested = cursor.sequence();
        let first_acknowledgeable = oldest.cursor.sequence().get().saturating_sub(1);
        if requested.get() < first_acknowledgeable {
            return Err(ReplayError::ReplayUnavailable(
                ReplayUnavailable::CursorTooOld {
                    requested: cursor.clone(),
                    oldest_available: oldest.cursor.clone(),
                },
            ));
        }
        if requested > newest.cursor.sequence() {
            return Err(ReplayError::ReplayUnavailable(
                ReplayUnavailable::CursorAhead {
                    requested: cursor.clone(),
                    newest_available: newest.cursor.clone(),
                },
            ));
        }
        Ok(self
            .entries
            .iter()
            .position(|entry| entry.cursor.sequence() > requested)
            .unwrap_or(self.entries.len()))
    }
}
