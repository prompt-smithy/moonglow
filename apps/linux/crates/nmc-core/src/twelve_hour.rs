//! Twelve-hour clock presentation.
//!
//! Ported from `Models/TwelveHourTime.swift`. This is a *presentation* type:
//! persistence and all schedule maths stay on the 24-hour [`TimeOfDay`], so a
//! format change can never alter a saved time.

use crate::TimeOfDay;

/// The half of the day an hour falls in, as shown by a twelve-hour clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Meridiem {
    Am,
    Pm,
}

impl Meridiem {
    /// `AM` / `PM`, as displayed next to the hour.
    pub const fn display_name(self) -> &'static str {
        match self {
            Meridiem::Am => "AM",
            Meridiem::Pm => "PM",
        }
    }

    /// The other half of the day.
    pub const fn toggled(self) -> Self {
        match self {
            Meridiem::Am => Meridiem::Pm,
            Meridiem::Pm => Meridiem::Am,
        }
    }

    /// First hour on the 24-hour clock for this half of the day.
    pub const fn start_hour(self) -> i32 {
        match self {
            Meridiem::Am => 0,
            Meridiem::Pm => 12,
        }
    }
}

/// A wall-clock time as read on a twelve-hour clock: `1...12`, minutes, meridiem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TwelveHourTime {
    pub hour: i32,
    pub minute: i32,
    pub meridiem: Meridiem,
}

impl TwelveHourTime {
    /// Hours in a valid twelve-hour reading: `1...12`, never `0`.
    pub const HOUR_RANGE: std::ops::RangeInclusive<i32> = 1..=12;
    /// Minutes in a valid reading.
    pub const MINUTE_RANGE: std::ops::RangeInclusive<i32> = 0..=59;

    pub const fn new(hour: i32, minute: i32, meridiem: Meridiem) -> Self {
        Self {
            hour,
            minute,
            meridiem,
        }
    }

    /// Creates a reading only when every component is in range.
    pub fn validating(hour: i32, minute: i32, meridiem: Meridiem) -> Option<Self> {
        Self::is_valid(hour, minute).then_some(Self::new(hour, minute, meridiem))
    }

    pub const fn is_valid(hour: i32, minute: i32) -> bool {
        // `RangeInclusive::contains` is not const on stable for all versions; compare directly.
        hour >= 1 && hour <= 12 && minute >= 0 && minute <= 59
    }

    pub const fn valid(&self) -> bool {
        Self::is_valid(self.hour, self.minute)
    }

    /// `6:00 AM` — no leading zero on the hour, minutes always two digits.
    pub fn display_text(&self) -> String {
        format!(
            "{}:{:02} {}",
            self.hour,
            self.minute,
            self.meridiem.display_name()
        )
    }

    /// Returns the same reading with a different meridiem, keeping the 12-hour
    /// value (so `11 AM` becomes `11 PM`, which is what a meridiem toggle does).
    pub const fn with_meridiem(&self, meridiem: Meridiem) -> Self {
        Self::new(self.hour, self.minute, meridiem)
    }
}

impl TimeOfDay {
    /// This time as a twelve-hour reading.
    ///
    /// `00:30` reads as `12:30 AM` and `12:30` as `12:30 PM`, which is the part
    /// a naive `hour % 12` conversion gets wrong.
    pub const fn twelve_hour(&self) -> TwelveHourTime {
        let meridiem = if self.hour < 12 {
            Meridiem::Am
        } else {
            Meridiem::Pm
        };
        let rem = self.hour % 12;
        let hour12 = if rem == 0 { 12 } else { rem };
        TwelveHourTime::new(hour12, self.minute, meridiem)
    }

    /// Builds a time from a twelve-hour reading. Returns `None` for an
    /// out-of-range reading rather than silently wrapping, so a bad picker
    /// value cannot shift the schedule by twelve hours.
    pub fn from_twelve_hour(t: TwelveHourTime) -> Option<Self> {
        if !t.valid() {
            return None;
        }
        let hour24 = match t.meridiem {
            Meridiem::Am => t.hour % 12,
            Meridiem::Pm => (t.hour % 12) + 12,
        };
        Some(Self::new(hour24, t.minute))
    }

    /// The reading to show for this time, always twelve-hour: `6:00 AM`.
    pub fn display_text(&self) -> String {
        self.twelve_hour().display_text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_around_noon_and_midnight() {
        assert_eq!(
            TimeOfDay::new(0, 30).twelve_hour(),
            TwelveHourTime::new(12, 30, Meridiem::Am)
        );
        assert_eq!(
            TimeOfDay::new(12, 30).twelve_hour(),
            TwelveHourTime::new(12, 30, Meridiem::Pm)
        );
        assert_eq!(
            TimeOfDay::new(23, 59).twelve_hour(),
            TwelveHourTime::new(11, 59, Meridiem::Pm)
        );
    }

    #[test]
    fn round_trips_all_1440_minutes() {
        for minutes in 0..1440 {
            let time = TimeOfDay::from_minutes(minutes);
            let back = TimeOfDay::from_twelve_hour(time.twelve_hour());
            assert_eq!(back, Some(time), "failed at {minutes} minutes");
        }
    }

    #[test]
    fn rejects_out_of_range_readings() {
        assert_eq!(
            TimeOfDay::from_twelve_hour(TwelveHourTime::new(0, 0, Meridiem::Am)),
            None
        );
        assert_eq!(
            TimeOfDay::from_twelve_hour(TwelveHourTime::new(13, 0, Meridiem::Pm)),
            None
        );
        assert_eq!(TimeOfDay::new(6, 5).display_text(), "6:05 AM");
        assert_eq!(TimeOfDay::new(18, 30).display_text(), "6:30 PM");
    }
}
