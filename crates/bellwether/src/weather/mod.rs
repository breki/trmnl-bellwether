//! Provider-neutral weather data and the
//! [`WeatherProvider`] trait.
//!
//! Concrete providers live under [`crate::clients`]
//! and each expose a type that implements
//! [`WeatherProvider`]. The dashboard model consumes
//! [`WeatherSnapshot`] directly and never sees a raw
//! wire format — all unit conversion happens inside
//! the provider adapter.
//!
//! See `docs/developer/weather-provider-migration.md`
//! for the design rationale and the PR sequence
//! that introduced this layer.
//!
//! ### Units
//!
//! | Field               | Unit                   |
//! |---------------------|------------------------|
//! | `temperature_c`     | degrees Celsius        |
//! | `humidity_pct`      | percent (0–100)        |
//! | `wind_kmh`          | km/h                   |
//! | `wind_dir_deg`      | compass deg (0 = N)    |
//! | `gust_kmh`          | km/h                   |
//! | `cloud_cover_pct`   | percent (0–100)        |
//! | `precip_mm`         | mm accumulated / step  |
//! | `weather_code`      | WMO 4677 code (0–99)   |
//!
//! All series are hourly and parallel to
//! [`WeatherSnapshot::timestamps`]. `None` entries
//! mark steps for which the provider had no datum
//! (e.g., grid edges, request clipping).

mod error;

#[cfg(test)]
mod tests;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

pub use error::{BoxError, WeatherError};

use crate::dashboard::astro::GeoPoint;
use crate::dashboard::classify::WeatherCode;

/// Hourly forecast data in display units, ready for
/// the dashboard model to consume without
/// conversion. See the module docs for the unit
/// table.
///
/// Fields are private to enforce the
/// "length-matches-timestamps" and "non-empty"
/// invariants at construction time. Providers build
/// snapshots through [`WeatherSnapshot::new`] (or
/// [`WeatherSnapshot::try_from_series`]) which
/// return `Err` on invariant violation. Tests and
/// adapters read the series via the accessors.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherSnapshot {
    timestamps: Vec<DateTime<Utc>>,
    temperature_c: Vec<Option<f64>>,
    humidity_pct: Vec<Option<f64>>,
    wind_kmh: Vec<Option<f64>>,
    wind_dir_deg: Vec<Option<f64>>,
    gust_kmh: Vec<Option<f64>>,
    cloud_cover_pct: Vec<Option<f64>>,
    precip_mm: Vec<Option<f64>>,
    weather_code: Vec<Option<WeatherCode>>,
    warning: Option<String>,
}

/// Builder for [`WeatherSnapshot`]. Holds the eight
/// hourly series plus an optional warning, and is
/// converted into a validated snapshot via
/// [`WeatherSnapshotBuilder::build`].
#[derive(Debug, Clone, Default)]
pub struct WeatherSnapshotBuilder {
    /// UTC timestamps for each hourly step.
    pub timestamps: Vec<DateTime<Utc>>,
    /// Air temperature, °C.
    pub temperature_c: Vec<Option<f64>>,
    /// Relative humidity, %.
    pub humidity_pct: Vec<Option<f64>>,
    /// Wind speed magnitude at 10 m, km/h.
    pub wind_kmh: Vec<Option<f64>>,
    /// Wind direction at 10 m, compass degrees
    /// (0 = N, clockwise).
    pub wind_dir_deg: Vec<Option<f64>>,
    /// Gust speed at 10 m, km/h.
    pub gust_kmh: Vec<Option<f64>>,
    /// Total cloud cover, %.
    pub cloud_cover_pct: Vec<Option<f64>>,
    /// Precipitation accumulated over the step, mm.
    pub precip_mm: Vec<Option<f64>>,
    /// Observed weather code for the step. `None` when
    /// the provider didn't supply one; `Some(Wmo(_))`
    /// for a recognised WMO 4677 code;
    /// `Some(Unrecognised(_))` when the wire carried a
    /// byte outside the documented subset (the
    /// display layer then surfaces
    /// [`ConditionCategory::Unknown`](crate::dashboard::classify::ConditionCategory::Unknown)).
    pub weather_code: Vec<Option<WeatherCode>>,
    /// Provider-supplied warning.
    pub warning: Option<String>,
}

impl WeatherSnapshotBuilder {
    /// Validate invariants and turn into a snapshot.
    ///
    /// Returns [`WeatherError::EmptySnapshot`] if
    /// `timestamps` is empty, or
    /// [`WeatherError::SeriesLengthMismatch`] if any
    /// series length differs from `timestamps.len()`.
    pub fn build(self) -> Result<WeatherSnapshot, WeatherError> {
        if self.timestamps.is_empty() {
            return Err(WeatherError::EmptySnapshot);
        }
        let expected = self.timestamps.len();
        let checks: [(&str, usize); 8] = [
            ("temperature_c", self.temperature_c.len()),
            ("humidity_pct", self.humidity_pct.len()),
            ("wind_kmh", self.wind_kmh.len()),
            ("wind_dir_deg", self.wind_dir_deg.len()),
            ("gust_kmh", self.gust_kmh.len()),
            ("cloud_cover_pct", self.cloud_cover_pct.len()),
            ("precip_mm", self.precip_mm.len()),
            ("weather_code", self.weather_code.len()),
        ];
        for (name, got) in checks {
            if got != expected {
                return Err(WeatherError::SeriesLengthMismatch {
                    series: name.to_owned(),
                    expected,
                    got,
                });
            }
        }
        Ok(WeatherSnapshot {
            timestamps: self.timestamps,
            temperature_c: self.temperature_c,
            humidity_pct: self.humidity_pct,
            wind_kmh: self.wind_kmh,
            wind_dir_deg: self.wind_dir_deg,
            gust_kmh: self.gust_kmh,
            cloud_cover_pct: self.cloud_cover_pct,
            precip_mm: self.precip_mm,
            weather_code: self.weather_code,
            warning: self.warning,
        })
    }
}

impl WeatherSnapshot {
    /// Alias for [`WeatherSnapshotBuilder::default`].
    #[must_use]
    pub fn builder() -> WeatherSnapshotBuilder {
        WeatherSnapshotBuilder::default()
    }

    /// UTC timestamps for each hourly step.
    #[must_use]
    pub fn timestamps(&self) -> &[DateTime<Utc>] {
        &self.timestamps
    }

    /// Air temperature, °C (None = no datum).
    #[must_use]
    pub fn temperature_c(&self) -> &[Option<f64>] {
        &self.temperature_c
    }

    /// Relative humidity, %.
    #[must_use]
    pub fn humidity_pct(&self) -> &[Option<f64>] {
        &self.humidity_pct
    }

    /// Wind speed at 10 m, km/h.
    #[must_use]
    pub fn wind_kmh(&self) -> &[Option<f64>] {
        &self.wind_kmh
    }

    /// Wind direction at 10 m, compass degrees.
    #[must_use]
    pub fn wind_dir_deg(&self) -> &[Option<f64>] {
        &self.wind_dir_deg
    }

    /// Gust speed at 10 m, km/h.
    #[must_use]
    pub fn gust_kmh(&self) -> &[Option<f64>] {
        &self.gust_kmh
    }

    /// Total cloud cover, %.
    #[must_use]
    pub fn cloud_cover_pct(&self) -> &[Option<f64>] {
        &self.cloud_cover_pct
    }

    /// Precipitation accumulated over the step, mm.
    #[must_use]
    pub fn precip_mm(&self) -> &[Option<f64>] {
        &self.precip_mm
    }

    /// Observed weather code for each step. See
    /// [`WeatherCode`] for the three-way semantics and
    /// [`classify_category`](crate::dashboard::classify::classify_category)
    /// for the display-layer dispatch. `None` entries
    /// mean "provider didn't supply" and route through
    /// the cloud+precip heuristic at display time.
    #[must_use]
    pub fn weather_code(&self) -> &[Option<WeatherCode>] {
        &self.weather_code
    }

    /// Provider-supplied warning (rate limit,
    /// degraded data, testing-tier notice, …).
    #[must_use]
    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }
}

/// A source of hourly weather data.
///
/// Implemented by each concrete provider. The trait
/// is object-safe (via `#[async_trait]`) so the
/// publish loop can hold an `Arc<dyn WeatherProvider>`.
#[async_trait]
pub trait WeatherProvider: Send + Sync {
    /// The geographic point this provider is
    /// configured to fetch forecasts for. The publish
    /// loop uses this for sunrise/sunset display so it
    /// doesn't need to carry a duplicate `GeoPoint`
    /// alongside the provider.
    fn location(&self) -> GeoPoint;

    /// Fetch the latest forecast. The returned
    /// snapshot was built through
    /// [`WeatherSnapshotBuilder::build`] so its length
    /// invariants are already enforced.
    async fn fetch(&self) -> Result<WeatherSnapshot, WeatherError>;
}
