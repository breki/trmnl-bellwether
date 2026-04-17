//! Runtime configuration for `bellwether`.
//!
//! The config lives in a TOML file. See
//! `docs/developer/spike.md` §7 for the schema decision
//! and `test-data/config-byos.toml` for an example.
//!
//! Secrets (API keys) are **not** stored inline. The
//! config holds a path to a file whose contents are the
//! secret. Relative paths are resolved against the
//! config file's parent directory so the config can be
//! relocated as a unit. Secret files are read eagerly
//! at [`Config::load`] time and kept in memory —
//! missing, unreadable, or empty secret files fail at
//! load rather than at first use.
//!
//! The module is split by config section: [`windy`],
//! [`trmnl`], and [`render`] each own their sub-types
//! and defaults. This module ties them together and
//! provides the load/parse entry points.

mod render;
mod trmnl;
mod windy;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub use self::render::{BitDepth, RenderConfig};
pub use self::trmnl::{ByosConfig, TrmnlConfig, WebhookConfig};
pub use self::windy::{WindyConfig, WindyParameter};

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
    /// Reading a referenced secret file failed.
    #[error("reading secret file {path}: {source}")]
    ReadSecret {
        /// Path we tried to read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A secret file existed but contained only
    /// whitespace.
    #[error("secret file {path} is empty")]
    EmptySecret {
        /// Path that was empty.
        path: PathBuf,
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
    /// `[windy] parameters` omits a parameter the v1
    /// dashboard requires. We reject at load time so an
    /// operator upgrading from a pre-0.8 config whose
    /// parameter list predates the full dashboard
    /// (which now consumes clouds and precip for
    /// condition classification, plus temp and wind for
    /// the labels) sees the migration path immediately
    /// — rather than silently rendering "Cloudy" on
    /// every tile because the classifier has nothing to
    /// work with.
    ///
    /// An empty `parameters` list is still accepted:
    /// that's how webhook-only deployments opt out of
    /// the Windy fetch entirely.
    #[error(
        "[windy] parameters is missing the dashboard's \
         required inputs {missing:?}; add them to the \
         list (v1 dashboard consumes temp, wind, \
         clouds, precip)"
    )]
    MissingRequiredWindyParameters {
        /// The subset of `REQUIRED_WINDY_PARAMETERS`
        /// the loaded config doesn't include.
        missing: Vec<WindyParameter>,
    },
}

/// Windy parameters the v1 dashboard needs. If
/// `[windy] parameters` is non-empty it must contain
/// all of these; if it is empty the check is skipped
/// (webhook-only deployments don't call Windy).
pub const REQUIRED_WINDY_PARAMETERS: &[WindyParameter] = &[
    WindyParameter::Temp,
    WindyParameter::Wind,
    WindyParameter::Clouds,
    WindyParameter::Precip,
];

/// Top-level configuration.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Windy Point Forecast settings.
    pub windy: WindyConfig,
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
    /// Does not resolve relative `api_key_file` paths
    /// or read secrets. Intended for tests, preview
    /// flows, and validation of user-supplied snippets.
    pub fn from_toml_str(toml_text: &str) -> Result<Self, ConfigError> {
        let cfg: Self = toml::from_str(toml_text).map_err(|source| {
            ConfigError::ParseToml {
                path: None,
                source: Box::new(source),
            }
        })?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Load, parse, validate, and eagerly bind secrets.
    ///
    /// Relative `api_key_file` paths are resolved
    /// against the config file's parent directory (or
    /// the current working directory if the config path
    /// has no directory component). The Windy API key
    /// file is read immediately and the trimmed
    /// contents are cached in memory so that misconfig
    /// fails fast at startup.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|source| {
            ConfigError::ReadConfig {
                path: path.to_path_buf(),
                source,
            }
        })?;
        let mut cfg: Self =
            toml::from_str(&text).map_err(|source| ConfigError::ParseToml {
                path: Some(path.to_path_buf()),
                source: Box::new(source),
            })?;
        let base = config_base_dir(path);
        cfg.windy.resolve_api_key_path(&base);
        cfg.validate()?;
        let key = read_secret(cfg.windy.api_key_file())?;
        cfg.windy.set_api_key(key);
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let lat = self.windy.lat;
        if !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
            return Err(ConfigError::InvalidLatitude(lat));
        }
        let lon = self.windy.lon;
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
        validate_windy_parameters(&self.windy.parameters)?;
        Ok(())
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "trmnl mode = {}, render = {}x{} @ {}-bit",
            self.trmnl,
            self.render.width,
            self.render.height,
            self.render.bit_depth.bits(),
        )
    }
}

fn config_base_dir(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

fn validate_windy_parameters(
    parameters: &[WindyParameter],
) -> Result<(), ConfigError> {
    if parameters.is_empty() {
        return Ok(());
    }
    let missing: Vec<WindyParameter> = REQUIRED_WINDY_PARAMETERS
        .iter()
        .filter(|p| !parameters.contains(p))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(ConfigError::MissingRequiredWindyParameters { missing });
    }
    Ok(())
}

pub(super) fn resolve_relative(base: &Path, p: &mut PathBuf) {
    if p.is_relative() {
        *p = base.join(&*p);
    }
}

fn read_secret(path: &Path) -> Result<String, ConfigError> {
    let raw =
        fs::read_to_string(path).map_err(|source| ConfigError::ReadSecret {
            path: path.to_path_buf(),
            source,
        })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::EmptySecret {
            path: path.to_path_buf(),
        });
    }
    Ok(trimmed.to_owned())
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
            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
        "#
    }

    #[test]
    fn loads_byos_config() {
        let path = fixture_dir().join("config-byos.toml");
        let cfg = Config::load(&path).unwrap();

        assert!((cfg.windy.lat - 46.05).abs() < 1e-9);
        assert!((cfg.windy.lon - 14.51).abs() < 1e-9);
        assert_eq!(cfg.windy.model, "gfs");
        assert_eq!(
            cfg.windy.parameters,
            vec![
                WindyParameter::Temp,
                WindyParameter::Wind,
                WindyParameter::Clouds,
                WindyParameter::Precip,
            ],
        );
        assert_eq!(cfg.windy.api_key(), Some("fake-windy-key-for-tests"));

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
        assert_eq!(cfg.windy.model, "gfs");
        assert!(cfg.windy.parameters.is_empty());
    }

    #[test]
    fn from_toml_str_parses_without_disk_io() {
        let cfg = Config::from_toml_str(minimal_byos_toml()).unwrap();
        assert_eq!(cfg.trmnl.mode_name(), "byos");
        assert!(cfg.windy.api_key().is_none());
    }

    #[test]
    fn resolves_relative_api_key_path_against_config_dir() {
        let path = fixture_dir().join("config-byos.toml");
        let cfg = Config::load(&path).unwrap();
        assert!(cfg.windy.api_key_file().is_absolute());
        assert!(cfg.windy.api_key_file().exists());
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
    fn reports_read_secret_error_for_missing_key_file() {
        let tmp = TempDir::new().unwrap();
        let cfg_path = tmp.path().join("c.toml");
        std::fs::write(
            &cfg_path,
            "[windy]\n\
             api_key_file = \"missing.txt\"\n\
             lat = 0\n\
             lon = 0\n\
             [trmnl]\n\
             mode = \"byos\"\n\
             public_image_base = \"http://x/\"\n",
        )
        .unwrap();
        let err = Config::load(&cfg_path).unwrap_err();
        assert!(matches!(err, ConfigError::ReadSecret { .. }));
    }

    #[test]
    fn rejects_empty_secret() {
        let tmp = TempDir::new().unwrap();
        let key_path = tmp.path().join("empty.txt");
        std::fs::write(&key_path, "   \n\t  \n").unwrap();
        let cfg_path = tmp.path().join("c.toml");
        std::fs::write(
            &cfg_path,
            format!(
                "[windy]\n\
                 api_key_file = \"{}\"\n\
                 lat = 0\n\
                 lon = 0\n\
                 [trmnl]\n\
                 mode = \"byos\"\n\
                 public_image_base = \"http://x/\"\n",
                key_path.file_name().unwrap().to_str().unwrap(),
            ),
        )
        .unwrap();
        let err = Config::load(&cfg_path).unwrap_err();
        assert!(matches!(err, ConfigError::EmptySecret { .. }));
    }

    #[test]
    fn rejects_mode_without_matching_payload() {
        let text = r#"
            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0

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

            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0

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
            [windy]
            api_key_file = "k.txt"
            lat = 200.0
            lon = 0.0

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
            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0

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
            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0

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
            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0

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
            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
            default_refresh_rate_s = 86401
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidRefreshRate(86401)));
    }

    #[test]
    fn rejects_byos_config_missing_required_windy_parameters() {
        // An operator upgrading from 0.7 whose
        // `parameters` list predates the v1 dashboard
        // should see a clear load-time error rather
        // than silently getting "Cloudy" on every
        // sample.
        let text = r#"
            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0
            parameters = ["temp", "wind", "precip"]

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        let ConfigError::MissingRequiredWindyParameters { missing } = err
        else {
            panic!("expected MissingRequiredWindyParameters, got {err:?}");
        };
        assert_eq!(missing, vec![WindyParameter::Clouds]);
    }

    #[test]
    fn accepts_empty_parameters_list_for_webhook_only_deployments() {
        // An empty parameters list is valid: webhook
        // mode never calls Windy, so nothing to
        // validate. The existing `loads_webhook_config`
        // test exercises this through the on-disk
        // fixture; pin it here too so the validation
        // rule change doesn't accidentally tighten
        // it.
        let text = r#"
            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"
        "#;
        Config::from_toml_str(text).expect("empty parameters is valid");
    }

    #[test]
    fn rejects_nan_longitude() {
        let text = r"
            [windy]
            api_key_file = 'k.txt'
            lat = 0.0
            lon = nan

            [trmnl]
            mode = 'byos'
            public_image_base = 'http://x/'
        ";
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidLongitude(_)));
    }

    #[test]
    fn display_uses_lowercase_mode() {
        let cfg = Config::from_toml_str(minimal_byos_toml()).unwrap();
        let s = format!("{cfg}");
        assert!(s.contains("trmnl mode = byos"));
        assert!(s.contains("800x480"));
        assert!(s.contains("1-bit"));
    }
}
