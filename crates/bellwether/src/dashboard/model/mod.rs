//! [`DashboardModel`] — presentation-ready view of a
//! [`crate::weather::WeatherSnapshot`].
//!
//! Folds the provider-neutral weather snapshot plus
//! device telemetry into what the SVG template needs:
//! current conditions, a summary for today, and three
//! forecast tiles for the next three calendar days.
//! Unit conversion has already happened in the
//! provider adapter before this module sees the data
//! — see `crate::weather` for the neutral types and
//! `crate::clients::open_meteo` for the current
//! Open-Meteo adapter.
//!
//! Split across two children:
//! - [`types`] — the presentation-data structs and
//!   shape constants.
//! - [`build`] — [`build_model`] and the pure
//!   builder helpers.

mod build;
#[cfg(test)]
mod tests;
mod types;

pub use build::build_model;
pub use types::{
    CurrentConditions, DAY_TILE_COUNT, DashboardModel, DaySummary,
    MIN_SAMPLES_PER_DAY, ModelContext, TodaySummary,
};

// Test-only re-export so tests can still reach the
// grouping helper they pin.
#[cfg(test)]
pub(crate) use build::group_sample_indices_by_date;
