//! Wall-clock time of day, independent of any calendar date.
//!
//! Ported from `Models/TimeOfDay.swift`. The persisted representation is a
//! zero-padded `HH:mm` string so settings files stay readable.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Minutes in a day; the modulus for [`TimeOfDay::from_minutes`].
pub const MINUTES_PER_DAY: i32 = 1440;

/// A wall-clock time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimeOfDay {
    /// Hour on the 24-hour clock, `0...23`.
    pub hour: i32,
    /// Minute, `0...59`.
    pub minute: i32,
}

impl TimeOfDay {
    /// Builds a time without validation. Prefer [`TimeOfDay::validating`] or
    /// [`TimeOfDay::from_str`] for untrusted input.
    pub const fn new(hour: i32, minute: i32) -> Self {
        Self { hour, minute }
    }

    /// Midnight, the start of the scheduling day.
    pub const MIDNIGHT: TimeOfDay = TimeOfDay::new(0, 0);

    /// Creates a time only when both components are in range.
    pub fn validating(hour: i32, minute: i32) -> Option<Self> {
        Self::is_valid(hour, minute).then_some(Self::new(hour, minute))
    }

    /// Whether the components form a valid time.
    pub const fn is_valid(hour: i32, minute: i32) -> bool {
        hour >= 0 && hour <= 23 && minute >= 0 && minute <= 59
    }

    /// Whether this time is valid.
    pub const fn valid(&self) -> bool {
        Self::is_valid(self.hour, self.minute)
    }

    /// Minutes elapsed since midnight. Used by schedule comparison and interpolation.
    pub const fn minutes_since_midnight(&self) -> i32 {
        self.hour * 60 + self.minute
    }

    /// A time built from minutes since midnight, wrapped into a single day.
    pub fn from_minutes(minutes: i32) -> Self {
        let wrapped = ((minutes % MINUTES_PER_DAY) + MINUTES_PER_DAY) % MINUTES_PER_DAY;
        Self::new(wrapped / 60, wrapped % 60)
    }

    /// `HH:mm` representation used for persistence.
    pub fn storage_description(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

impl FromStr for TimeOfDay {
    type Err = InvalidTimeOfDay;

    /// Parses a zero-padded or bare `H:mm` string.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (h, m) = s.split_once(':').ok_or(InvalidTimeOfDay)?;
        let hour: i32 = h.parse().map_err(|_| InvalidTimeOfDay)?;
        let minute: i32 = m.parse().map_err(|_| InvalidTimeOfDay)?;
        Self::validating(hour, minute).ok_or(InvalidTimeOfDay)
    }
}

impl fmt::Display for TimeOfDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.storage_description())
    }
}

/// Rejection for [`TimeOfDay::from_str`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTimeOfDay;

impl fmt::Display for InvalidTimeOfDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected a \"HH:mm\" time")
    }
}

impl std::error::Error for InvalidTimeOfDay {}

impl Serialize for TimeOfDay {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.storage_description())
    }
}

impl<'de> Deserialize<'de> for TimeOfDay {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::from_str(&raw).map_err(|_| {
            serde::de::Error::custom(format!("expected a \"HH:mm\" time, received \"{raw}\""))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_storage_form() {
        let time = TimeOfDay::new(6, 5);
        assert_eq!(time.storage_description(), "06:05");
        assert_eq!(TimeOfDay::from_str("06:05"), Ok(time));
        assert_eq!(TimeOfDay::from_str("6:05"), Ok(time));
        assert_eq!(TimeOfDay::from_str("24:00"), Err(InvalidTimeOfDay));
        assert_eq!(TimeOfDay::from_str("06:60"), Err(InvalidTimeOfDay));
        assert_eq!(TimeOfDay::from_str("not-a-time"), Err(InvalidTimeOfDay));
    }

    #[test]
    fn wraps_minutes_into_single_day() {
        assert_eq!(TimeOfDay::from_minutes(0), TimeOfDay::MIDNIGHT);
        assert_eq!(TimeOfDay::from_minutes(1439), TimeOfDay::new(23, 59));
        assert_eq!(TimeOfDay::from_minutes(1440), TimeOfDay::MIDNIGHT);
        assert_eq!(TimeOfDay::from_minutes(-1), TimeOfDay::new(23, 59));
    }

    #[test]
    fn orders_by_minutes_since_midnight() {
        assert!(TimeOfDay::new(6, 0) < TimeOfDay::new(18, 30));
        assert_eq!(TimeOfDay::new(6, 0).minutes_since_midnight(), 360);
    }
}
