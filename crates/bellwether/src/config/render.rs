//! Render pipeline configuration sub-module.

use serde::Deserialize;

/// Rendering pipeline configuration.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct RenderConfig {
    /// Output image width in pixels.
    #[serde(default = "default_width")]
    pub width: u32,
    /// Output image height in pixels.
    #[serde(default = "default_height")]
    pub height: u32,
    /// Output bit depth.
    #[serde(default)]
    pub bit_depth: BitDepth,
    /// IANA timezone used for rendered clocks and date
    /// labels.
    #[serde(default = "default_timezone")]
    pub timezone: chrono_tz::Tz,
}

impl Default for RenderConfig {
    fn default() -> Self {
        // Synthesise via serde so field-level defaults
        // stay the single source of truth.
        toml::from_str("").expect(
            "empty TOML must deserialize into RenderConfig via defaults",
        )
    }
}

/// Output bit depth. Restricted to the values the TRMNL
/// hardware supports.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy, Default)]
#[serde(try_from = "u8")]
#[non_exhaustive]
pub enum BitDepth {
    /// 1-bit black/white (TRMNL OG).
    #[default]
    One,
    /// 4-bit (16-level) grayscale (TRMNL X).
    Four,
}

impl BitDepth {
    /// Returns the bit count as the integer the user
    /// wrote in their TOML.
    pub fn bits(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Four => 4,
        }
    }
}

impl TryFrom<u8> for BitDepth {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::One),
            4 => Ok(Self::Four),
            other => Err(format!("bit_depth must be 1 or 4, got {other}")),
        }
    }
}

fn default_width() -> u32 {
    800
}

fn default_height() -> u32 {
    480
}

fn default_timezone() -> chrono_tz::Tz {
    chrono_tz::UTC
}

#[cfg(test)]
mod tests {
    use super::super::{Config, ConfigError};
    use super::*;

    #[test]
    fn default_matches_serde_defaults() {
        let via_default = RenderConfig::default();
        let via_empty_toml: RenderConfig = toml::from_str("").unwrap();
        assert_eq!(via_default, via_empty_toml);
    }

    #[test]
    fn rejects_invalid_bit_depth() {
        let text = r#"
            [windy]
            api_key_file = "k.txt"
            lat = 0
            lon = 0

            [trmnl]
            mode = "byos"
            public_image_base = "http://x/"

            [render]
            bit_depth = 7
        "#;
        let err = Config::from_toml_str(text).unwrap_err();
        assert!(matches!(err, ConfigError::ParseToml { .. }));
    }
}
