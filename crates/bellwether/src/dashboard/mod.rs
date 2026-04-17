//! Dashboard composition layer.
//!
//! Interprets [`crate::clients::windy::Forecast`] data
//! into a presentation-ready [`DashboardModel`], then
//! renders it to an SVG string the [`crate::render`]
//! pipeline can turn into a 1-bit BMP.
//!
//! This module owns the weather-domain decisions: which
//! cloud-cover percentages count as "sunny", how to
//! convert Windy's `wind_u`/`wind_v` components into a
//! human-facing "Wind 8 km/h NW" label, how forecast
//! timestamps bucket into local calendar days, and what
//! the dashboard should say when data is partially
//! missing. Pure-function-heavy by design so every
//! interpretation decision is directly table-testable
//! without going through the rasterizer.
//!
//! The [`render`](crate::render) module stays a thin
//! SVG → BMP transport; it doesn't know a sunny day
//! from a rainy one.

pub mod astro;
pub mod classify;
pub mod feels_like;
pub mod icons;
pub mod model;
pub mod svg;

pub use classify::{Compass8, Condition, classify_weather, wind_to_compass};
pub use model::{
    CurrentConditions, DAY_TILE_COUNT, DashboardModel, DaySummary,
    MIN_SAMPLES_PER_DAY, ModelContext, TodaySummary, build_model,
};
pub use svg::build_svg;
