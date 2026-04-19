//! Runtime configuration for `bellwether`.
//!
//! The config lives in a TOML file. See
//! `docs/developer/spike.md` §7 for the schema decision
//! and `test-data/config-byos.toml` for an example.
//!
//! The module is split by config section: [`weather`],
//! [`trmnl`], and [`render`] each own their sub-types
//! and defaults. This module ties them together and
//! provides the load/parse entry points.

mod render;
mod trmnl;
mod weather;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub use self::render::{BitDepth, RenderConfig};
pub use self::trmnl::{ByosConfig, TrmnlConfig, WebhookConfig};
pub use self::weather::{OpenMeteoProviderConfig, ProviderKind, WeatherConfig};

/// Errors returned by [`Config::load`] and
/// [`Config::from_toml_str`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// Reading the config TOML failed.
    #[error("reading config file {path}: {source}")]
    ReadConfig {
        /// Path we tried to read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The TOML was malformed or had unexpected shape.
    /// `path` is `None` when parsing from an in-memory
    /// string via [`Config::from_toml_str`].
    #[error("parsing TOML config{}: {source}",
        .path.as_ref().map(|p| format!(" {}", p.display()))
            .unwrap_or_default())]
    ParseToml {
        /// Config file, if loaded from disk.
        path: Option<PathBuf>,
        /// Underlying parse error (boxed — the raw
        /// `toml::de::Error` is >128 bytes).
        #[source]
        source: Box<toml::de::Error>,
    },
    /// Latitude was out of `[-90, 90]` or not finite.
    #[error("invalid latitude {0}: must be finite and in [-90, 90]")]
    InvalidLatitude(f64),
    /// Longitude was out of `[-180, 180]` or not finite.
    #[error("invalid longitude {0}: must be finite and in [-180, 180]")]
    InvalidLongitude(f64),
    /// Render dimension outside the supported range.
    /// Bounds are deliberately loose (4096 px per axis)
    /// so future devices can raise them without a
    /// `SemVer` break; today's TRMNL X tops out at
    /// 1872 px.
    #[error(
        "invalid render dimensions {width}x{height}: each \
         must be in 1..=4096"
    )]
    InvalidRenderDimensions {
        /// Requested width in pixels.
        width: u32,
        /// Requested height in pixels.
        height: u32,
    },
    /// BYOS refresh rate outside the supported range.
    /// Zero panics `tokio::time::interval`; anything
    /// above a day makes no sense for an e-ink
    /// dashboard.
    #[error(
        "invalid BYOS default_refresh_rate_s {0}: must \
         be in 1..=86400"
    )]
    InvalidRefreshRate(u32),
    /// `[weather] provider = "<name>"` but the matching
    /// `[weather.<name>]` subtable is missing. We reject
    /// at load time so a misconfigured file fails at
    /// startup rather than surfacing as a runtime panic
    /// when the publish loop tries to build a request.
    #[error(
        "[weather] provider = \"{provider}\" but the \
         `[weather.{provider}]` subtable is missing"
    )]
    MissingProviderSubtable {
        /// The provider tag that named the missing
        /// subtable.
        provider: &'static str,
    },
}

/// Top-level configuration.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Weather provider settings.
    pub weather: WeatherConfig,
    /// TRMNL publishing settings (discriminated union
    /// by `mode`).
    pub trmnl: TrmnlConfig,
    /// Rendering pipeline settings.
    #[serde(default)]
    pub render: RenderConfig,
}

impl Config {
    /// Parse a config from an in-memory TOML string.
    ///
    /// Intended for tests, preview flows, and
    /// validation of user-supplied snippets.
    pub fn from_toml_str(toml_text: &str) -> Result<Self, ConfigError> {
        parse_and_validate(toml_text, None)
    }

    /// Load and parse the config from disk and
    /// validate it.
    ///
    /// The only provider currently shipped
    /// (Open-Meteo) needs no on-disk secrets, so
    /// `load` is a thin wrapper around TOML parsing.
    /// The infrastructure for reading referenced
    /// secret files was removed when the Windy
    /// provider went away; reinstate it from git
    /// history if a future provider needs it.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|source| {
            ConfigError::ReadConfig {
                path: path.to_path_buf(),
                source,
            }
        })?;
        parse_and_validate(&text, Some(path))
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let lat = self.weather.lat;
        if !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
            return Err(ConfigError::InvalidLatitude(lat));
        }
        let lon = self.weather.lon;
        if !lon.is_finite() || !(-180.0..=180.0).contains(&lon) {
            return Err(ConfigError::InvalidLongitude(lon));
        }
        let (w, h) = (self.render.width, self.render.height);
        if !(1..=4096).contains(&w) || !(1..=4096).contains(&h) {
            return Err(ConfigError::InvalidRenderDimensions {
                width: w,
                height: h,
            });
        }
        if let TrmnlConfig::Byos(byos) = &self.trmnl
            && !(1..=86400).contains(&byos.default_refresh_rate_s)
        {
            return Err(ConfigError::InvalidRefreshRate(
                byos.default_refresh_rate_s,
            ));
        }
        self.validate_active_provider()?;
        Ok(())
    }

    fn validate_active_provider(&self) -> Result<(), ConfigError> {
        match self.weather.provider {
            ProviderKind::OpenMeteo => {
                self.weather.open_meteo.as_ref().ok_or(
                    ConfigError::MissingProviderSubtable {
                        provider: ProviderKind::OpenMeteo.name(),
                    },
                )?;
            }
        }
        Ok(())
    }
}

/// Shared implementation behind
/// [`Config::from_toml_str`] and [`Config::load`] —
/// parses the TOML with the appropriate error context
/// then runs [`Config::validate`].
fn parse_and_validate(
    toml_text: &str,
    path: Option<&Path>,
) -> Result<Config, ConfigError> {
    let cfg: Config =
        toml::from_str(toml_text).map_err(|source| ConfigError::ParseToml {
            path: path.map(Path::to_path_buf),
            source: Box::new(source),
        })?;
    cfg.validate()?;
    Ok(cfg)
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "trmnl mode = {}, weather provider = {}, \
             render = {}x{} @ {}-bit",
            self.trmnl,
            self.weather.provider.name(),
            self.render.width,
            self.render.height,
            self.render.bit_depth.bits(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-data")
    }

    fn minimal_byos_toml() -> &'static str {
        r#"
            [weather]
            provider = "open_meteo"
            lat = 0
            lon = 0

            [weather.open_meteo]

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
        "#
    }

    fn open_meteo_subtable(cfg: &Config) -> &OpenMeteoProviderConfig {
        cfg.weather
            .open_meteo
            .as_ref()
            .expect("open_meteo subtable")
    }

    #[test]
    fn loads_byos_config() {
        let path = fixture_dir().join("config-byos.toml");
        let cfg = Config::load(&path).unwrap();

        assert_eq!(cfg.weather.provider, ProviderKind::OpenMeteo);
        assert!((cfg.weather.lat - 46.05).abs() < 1e-9);
        assert!((cfg.weather.lon - 14.51).abs() < 1e-9);
        let om = open_meteo_subtable(&cfg);
        assert_eq!(om.model, "icon_eu");

        match &cfg.trmnl {
            TrmnlConfig::Byos(byos) => {
                assert_eq!(
                    byos.public_image_base,
                    "http://malina.local:3100/images",
                );
                assert_eq!(byos.default_refresh_rate_s, 900);
            }
            other => panic!("expected Byos, got {other:?}"),
        }

        assert_eq!(cfg.render.width, 800);
        assert_eq!(cfg.render.height, 480);
        assert_eq!(cfg.render.bit_depth, BitDepth::One);
        assert_eq!(cfg.render.timezone, chrono_tz::Europe::Ljubljana);
    }

    #[test]
    fn loads_webhook_config_with_defaults() {
        let path = fixture_dir().join("config-webhook.toml");
        let cfg = Config::load(&path).unwrap();

        match &cfg.trmnl {
            TrmnlConfig::Webhook(webhook) => {
                assert_eq!(webhook.content_type, "image/bmp");
            }
            other => panic!("expected Webhook, got {other:?}"),
        }

        assert_eq!(cfg.render, RenderConfig::default());
        let om = open_meteo_subtable(&cfg);
        assert_eq!(om.model, "icon_eu");
    }

    #[test]
    fn from_toml_str_parses_without_disk_io() {
        let cfg = Config::from_toml_str(minimal_byos_toml()).unwrap();
        assert_eq!(cfg.trmnl.mode_name(), "byos");
    }

    #[test]
    fn rejects_malformed_toml() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "this is = not [valid").unwrap();
        let err = Config::load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::ParseToml { .. }));
    }

    #[test]
    fn reports_read_config_error_for_missing_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nope.toml");
        let err = Config::load(&path).unwrap_err();
        let ConfigError::ReadConfig { path: p, .. } = err else {
            panic!("expected ReadConfig")
        };
        assert_eq!(p, path);
    }

    #[test]
    fn rejects_mode_without_matching_payload() {
        let text = r#"
            [weather]
            provider = "open_meteo"
            lat = 0
            lon = 0

            [weather.open_meteo]

            [trmnl]
            mode = "byos"
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(err, ConfigError::ParseToml { .. }));
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let text = r#"
            mystery = "field"

            [weather]
            provider = "open_meteo"
            lat = 0
            lon = 0

            [weather.open_meteo]

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(err, ConfigError::ParseToml { .. }));
    }

    #[test]
    fn rejects_out_of_range_latitude() {
        let text = r#"
            [weather]
            provider = "open_meteo"
            lat = 200.0
            lon = 0.0

            [weather.open_meteo]

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidLatitude(_)));
    }

    #[test]
    fn rejects_out_of_range_render_dimensions() {
        let text = r#"
            [weather]
            provider = "open_meteo"
            lat = 0
            lon = 0

            [weather.open_meteo]

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"

            [render]
            width = 65535
            height = 65535
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::InvalidRenderDimensions {
                width: 65535,
                height: 65535,
            },
        ));
    }

    #[test]
    fn rejects_zero_render_dimension() {
        let text = r#"
            [weather]
            provider = "open_meteo"
            lat = 0
            lon = 0

            [weather.open_meteo]

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"

            [render]
            width = 0
            height = 480
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(
            err,
            ConfigError::InvalidRenderDimensions {
                width: 0,
                height: 480,
            },
        ));
    }

    #[test]
    fn rejects_zero_refresh_rate() {
        let text = r#"
            [weather]
            provider = "open_meteo"
            lat = 0
            lon = 0

            [weather.open_meteo]

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
            default_refresh_rate_s = 0
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidRefreshRate(0)));
    }

    #[test]
    fn rejects_too_large_refresh_rate() {
        let text = r#"
            [weather]
            provider = "open_meteo"
            lat = 0
            lon = 0

            [weather.open_meteo]

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
            default_refresh_rate_s = 86401
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidRefreshRate(86401)));
    }

    #[test]
    fn rejects_open_meteo_provider_without_subtable() {
        let text = r#"
            [weather]
            provider = "open_meteo"
            lat = 0
            lon = 0

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        let ConfigError::MissingProviderSubtable { provider } = err else {
            panic!("expected MissingProviderSubtable, got {err:?}");
        };
        assert_eq!(provider, "open_meteo");
    }

    #[test]
    fn rejects_nan_longitude() {
        let text = r"
            [weather]
            provider = 'open_meteo'
            lat = 0.0
            lon = nan

            [weather.open_meteo]

            [trmnl]
            mode = 'byos'
            public_image_base = 'http://x/'
        ";
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidLongitude(_)));
    }

    #[test]
    fn display_uses_lowercase_mode_and_provider() {
        let cfg = Config::from_toml_str(minimal_byos_toml()).unwrap();
        let s = format!("{cfg}");
        assert!(s.contains("trmnl mode = byos"));
        assert!(s.contains("weather provider = open_meteo"));
        assert!(s.contains("800x480"));
        assert!(s.contains("1-bit"));
    }
}
