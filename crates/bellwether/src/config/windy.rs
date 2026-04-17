//! Windy Point Forecast configuration sub-module.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Windy Point Forecast configuration.
///
/// The API key is read from `api_key_file` at
/// [`Config::load`](super::Config::load) time and kept
/// in memory. It is not deserialized from TOML — the
/// field `api_key` is `#[serde(skip)]`.
#[derive(Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WindyConfig {
    /// Path to a file whose contents are the API key.
    /// Relative paths are resolved against the config
    /// file's parent directory.
    api_key_file: PathBuf,
    /// Point latitude in decimal degrees.
    pub lat: f64,
    /// Point longitude in decimal degrees.
    pub lon: f64,
    /// Forecast model name (Windy's `model` parameter).
    #[serde(default = "default_windy_model")]
    pub model: String,
    /// Forecast parameters to request.
    #[serde(default)]
    pub parameters: Vec<WindyParameter>,
    /// Resolved API key; populated by `Config::load`,
    /// `None` after `Config::from_toml_str`.
    #[serde(skip)]
    api_key: Option<String>,
}

impl WindyConfig {
    /// Returns the secret-file path (resolved to
    /// absolute after [`Config::load`]).
    pub fn api_key_file(&self) -> &Path {
        &self.api_key_file
    }

    /// Returns the API key resolved at load time, if
    /// any.
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    pub(super) fn set_api_key(&mut self, key: String) {
        self.api_key = Some(key);
    }

    pub(super) fn resolve_api_key_path(&mut self, base: &Path) {
        super::resolve_relative(base, &mut self.api_key_file);
    }
}

impl fmt::Debug for WindyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindyConfig")
            .field("api_key_file", &self.api_key_file)
            .field("lat", &self.lat)
            .field("lon", &self.lon)
            .field("model", &self.model)
            .field("parameters", &self.parameters)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// Windy Point Forecast parameters. The closed set
/// supported by Windy's API — add variants as the
/// forecast client starts consuming them. Serde names
/// match Windy's wire format (camelCase for compound
/// names, lowercase otherwise).
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
#[non_exhaustive]
pub enum WindyParameter {
    /// Air temperature.
    #[serde(rename = "temp")]
    Temp,
    /// Wind speed and direction (u/v components).
    #[serde(rename = "wind")]
    Wind,
    /// Wind gusts.
    #[serde(rename = "windGust")]
    WindGust,
    /// Precipitation.
    #[serde(rename = "precip")]
    Precip,
    /// Surface pressure.
    #[serde(rename = "pressure")]
    Pressure,
    /// Cloud cover.
    #[serde(rename = "clouds")]
    Clouds,
    /// Relative humidity.
    #[serde(rename = "rh")]
    Rh,
    /// Dewpoint temperature.
    #[serde(rename = "dewpoint")]
    Dewpoint,
}

impl WindyParameter {
    /// Wire name as used in Windy requests (JSON
    /// `parameters` array) and response series keys
    /// (`"{wire_name}-{level}"`). Kept in sync with the
    /// `#[serde(rename)]` attributes above; a unit test
    /// asserts equivalence.
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Temp => "temp",
            Self::Wind => "wind",
            Self::WindGust => "windGust",
            Self::Precip => "precip",
            Self::Pressure => "pressure",
            Self::Clouds => "clouds",
            Self::Rh => "rh",
            Self::Dewpoint => "dewpoint",
        }
    }
}

fn default_windy_model() -> String {
    "gfs".to_owned()
}

#[cfg(test)]
mod tests {
    use super::super::Config;
    use super::*;

    #[test]
    fn wire_name_matches_serde_rename() {
        // Lock the invariant: `wire_name()` and the
        // #[serde(rename)] values must agree, else the
        // Windy client's request bodies disagree with
        // its response-series key lookups.
        for p in [
            WindyParameter::Temp,
            WindyParameter::Wind,
            WindyParameter::WindGust,
            WindyParameter::Precip,
            WindyParameter::Pressure,
            WindyParameter::Clouds,
            WindyParameter::Rh,
            WindyParameter::Dewpoint,
        ] {
            let serialized = serde_json::to_string(&p).unwrap();
            assert_eq!(serialized, format!("\"{}\"", p.wire_name()));
        }
    }

    #[test]
    fn debug_redacts_api_key() {
        let mut cfg = Config::from_toml_str(
            r#"
            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
            "#,
        )
        .unwrap();
        cfg.windy.set_api_key("super-secret-key".to_owned());
        let s = format!("{:?}", cfg.windy);
        assert!(s.contains("<redacted>"));
        assert!(!s.contains("super-secret-key"));
    }
}
