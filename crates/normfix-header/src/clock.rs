//! One-shot, reproducible clock used by official headers.

use std::env;

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use thiserror::Error;

const HEADER_FORMAT: &str = "%Y/%m/%d %H:%M:%S";

/// Origin of a captured run timestamp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockSource {
    /// Current local wall clock, captured once.
    SystemLocal,
    /// UTC instant supplied through `SOURCE_DATE_EPOCH`.
    SourceDateEpoch,
    /// Explicit timestamp, normally used by a deterministic caller or test.
    Fixed,
}

/// A timestamp captured once and reused by every file in a transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunClock {
    timestamp: String,
    source: ClockSource,
}

impl RunClock {
    /// Captures the local wall clock once.
    #[must_use]
    pub fn system_local() -> Self {
        Self {
            timestamp: Local::now().format(HEADER_FORMAT).to_string(),
            source: ClockSource::SystemLocal,
        }
    }

    /// Reads `SOURCE_DATE_EPOCH`, falling back to one local wall-clock sample.
    ///
    /// An explicitly present but invalid environment value is an error; it is
    /// never silently replaced with the current time.
    ///
    /// # Errors
    ///
    /// Returns an error when `SOURCE_DATE_EPOCH` is non-Unicode, malformed or
    /// outside the supported timestamp range.
    pub fn from_process_environment() -> Result<Self, ClockError> {
        match env::var("SOURCE_DATE_EPOCH") {
            Ok(value) => Self::from_source_date_epoch(&value),
            Err(env::VarError::NotPresent) => Ok(Self::system_local()),
            Err(env::VarError::NotUnicode(_)) => Err(ClockError::NonUnicodeSourceDateEpoch),
        }
    }

    /// Creates a UTC clock from a decimal `SOURCE_DATE_EPOCH` value.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-decimal or out-of-range Unix timestamp.
    pub fn from_source_date_epoch(value: &str) -> Result<Self, ClockError> {
        let seconds = value
            .parse::<i64>()
            .map_err(|_| ClockError::InvalidSourceDateEpoch(value.to_owned()))?;
        let datetime = DateTime::<Utc>::from_timestamp(seconds, 0)
            .ok_or(ClockError::TimestampOutOfRange(seconds))?;
        Ok(Self {
            timestamp: datetime.format(HEADER_FORMAT).to_string(),
            source: ClockSource::SourceDateEpoch,
        })
    }

    /// Creates a deterministic clock from an official-header timestamp.
    ///
    /// # Errors
    ///
    /// Returns an error unless the value is a real date and time written as
    /// `YYYY/MM/DD HH:MM:SS`.
    pub fn fixed(timestamp: &str) -> Result<Self, ClockError> {
        NaiveDateTime::parse_from_str(timestamp, HEADER_FORMAT)
            .map_err(|_| ClockError::InvalidFixedTimestamp(timestamp.to_owned()))?;
        Ok(Self {
            timestamp: timestamp.to_owned(),
            source: ClockSource::Fixed,
        })
    }

    /// Returns the official-header timestamp (`YYYY/MM/DD HH:MM:SS`).
    #[must_use]
    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }

    /// Returns where the run timestamp came from.
    #[must_use]
    pub const fn source(&self) -> ClockSource {
        self.source
    }
}

/// Invalid deterministic clock input.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ClockError {
    /// `SOURCE_DATE_EPOCH` was not valid UTF-8.
    #[error("SOURCE_DATE_EPOCH is not valid UTF-8")]
    NonUnicodeSourceDateEpoch,
    /// The environment value was not a decimal Unix timestamp.
    #[error("SOURCE_DATE_EPOCH must be a decimal Unix timestamp, got `{0}`")]
    InvalidSourceDateEpoch(String),
    /// Chrono could not represent the Unix timestamp.
    #[error("SOURCE_DATE_EPOCH timestamp {0} is outside the supported range")]
    TimestampOutOfRange(i64),
    /// The fixed timestamp did not have the exact official-header shape.
    #[error("fixed timestamp must use YYYY/MM/DD HH:MM:SS, got `{0}`")]
    InvalidFixedTimestamp(String),
}

#[cfg(test)]
mod tests {
    use super::{ClockSource, RunClock};

    #[test]
    fn source_date_epoch_is_utc_and_reproducible() {
        let clock = RunClock::from_source_date_epoch("1782218096").expect("valid epoch");
        assert_eq!(clock.timestamp(), "2026/06/23 12:34:56");
        assert_eq!(clock.source(), ClockSource::SourceDateEpoch);
    }

    #[test]
    fn fixed_timestamp_requires_the_exact_shape() {
        assert!(RunClock::fixed("2026/07/23 12:34:56").is_ok());
        assert!(RunClock::fixed("2026-07-23T12:34:56").is_err());
    }
}
