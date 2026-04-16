//! Windy Point Forecast configuration sub-module.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

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
/// forecast client starts consuming them.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum WindyParameter {
    /// Air temperature.
    Temp,
    /// Wind speed and direction.
    Wind,
    /// Wind gusts.
    WindGust,
    /// Precipitation.
    Precip,
    /// Surface pressure.
    Pressure,
    /// Cloud cover.
    Clouds,
    /// Relative humidity.
    Rh,
    /// Dewpoint temperature.
    Dewpoint,
}

fn default_windy_model() -> String {
    "gfs".to_owned()
}

#[cfg(test)]
mod tests {
    use super::super::Config;

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
