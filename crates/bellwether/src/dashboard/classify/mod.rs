//! Display-layer classification and bucketing.
//!
//! Two sibling submodules that share no types, tests,
//! or invariants — they coexist under this module name
//! only because both are pure "display-layer bucketing"
//! over already-normalised provider inputs.
//!
//! - [`weather`] — weather-state taxonomy. Two-tier
//!   design around [`WmoCode`], [`ConditionCategory`],
//!   [`WeatherCode`], and the composite entry point
//!   [`classify_category`]. See the module doc in
//!   `weather.rs` for the full story.
//! - [`compass`] — eight-way bucketing of a wind
//!   direction in degrees for the dashboard's wind
//!   label.
//!
//! Unit conversion and u/v → (magnitude, direction)
//! arithmetic live in the provider adapter (see
//! `crate::clients::open_meteo`).

mod compass;
mod weather;

pub use compass::Compass8;
pub use weather::{
    ConditionCategory, RAIN_THRESHOLD_MMH, UnknownWmoCode, WeatherCode,
    WmoCode, classify_category,
};
