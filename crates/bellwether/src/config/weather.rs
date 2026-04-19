//! Weather configuration sub-module.
//!
//! The `[weather]` table holds provider-neutral fields
//! (lat, lon, provider tag) plus a provider-specific
//! subtable like `[weather.open_meteo]`. See
//! `docs/developer/weather-provider-migration.md` for
//! the rationale behind the tagged layout.

use serde::Deserialize;

/// Top-level weather configuration: which provider to
/// use, where the point of interest is, plus an
/// optional subtable per known provider.
///
/// Only the subtable matching `provider` is required;
/// the others may be present (handy for keeping
/// credentials configured while experimenting) but are
/// ignored at runtime.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WeatherConfig {
    /// Active provider. Determines which subtable
    /// must be populated and which concrete
    /// `WeatherProvider` the publish loop builds.
    pub provider: ProviderKind,
    /// Point latitude in decimal degrees.
    pub lat: f64,
    /// Point longitude in decimal degrees.
    pub lon: f64,
    /// Open-Meteo provider subtable. Required when
    /// `provider = "open_meteo"`.
    #[serde(default)]
    pub open_meteo: Option<OpenMeteoProviderConfig>,
}

/// Closed set of providers the config layer knows
/// about. Adding a new provider means adding a
/// variant here plus a matching subtable field on
/// [`WeatherConfig`].
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Open-Meteo forecast API. Free and keyless.
    OpenMeteo,
}

impl ProviderKind {
    /// Stable short name used in error messages and
    /// log lines (matches the `#[serde(rename)]`
    /// values).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenMeteo => "open_meteo",
        }
    }
}

/// Open-Meteo provider configuration.
///
/// Open-Meteo needs no API key — the config only
/// carries the forecast-model selection. Defaults to
/// `icon_eu` (DWD's European model, ~6 km resolution,
/// updated 4×/day) because the reference deployment
/// is in Slovenia. Change `model` for other regions:
/// `best_match` (auto-select), `gfs_global` (25 km
/// worldwide), or any model Open-Meteo supports.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OpenMeteoProviderConfig {
    /// Forecast model to request. See
    /// <https://open-meteo.com/en/docs> for the
    /// full list of models the API accepts.
    #[serde(default = "default_openmeteo_model")]
    pub model: String,
}

fn default_openmeteo_model() -> String {
    "icon_eu".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_name_matches_serde() {
        assert_eq!(ProviderKind::OpenMeteo.name(), "open_meteo");
        let parsed: ProviderKind =
            serde_json::from_str("\"open_meteo\"").unwrap();
        assert_eq!(parsed, ProviderKind::OpenMeteo);
    }

    #[test]
    fn open_meteo_model_has_icon_eu_default() {
        let cfg: OpenMeteoProviderConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.model, "icon_eu");
    }
}
