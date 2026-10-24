//! Platform logic for Moonglow, ported from Swift `MoonglowKit`.
//!
//! Models, settings, schedule math, Kelvin conversion, DDC/CI packets, and the
//! headless [`StateManager`] (the `MainViewModel` equivalent). This crate has no
//! UI framework or OS backend dependency; Linux backends live in the app crate.

pub mod ddc;
pub mod display_info;
pub mod kelvin;
pub mod schedule;
pub mod settings;
pub mod shortcut;
pub mod state;
pub mod temperature_range;
pub mod theme;
pub mod time_of_day;
pub mod traits;
pub mod twelve_hour;

pub use display_info::DisplayInfo;
pub use schedule::{ScheduleCalculator, ScheduleService, TICK_INTERVAL_SECS};
pub use settings::AppSettings;
pub use shortcut::{Shortcut, ShortcutAction};
pub use state::StateManager;
pub use temperature_range::{TemperatureRange, TemperatureScale};
pub use time_of_day::TimeOfDay;
pub use twelve_hour::{Meridiem, TwelveHourTime};
