//! TRMNL publishing configuration sub-module.
//!
//! `TrmnlConfig` is an internally-tagged enum
//! discriminated by `mode`. Variant-specific fields
//! live directly under the `[trmnl]` table; there is
//! no nested `[trmnl.byos]` or `[trmnl.webhook]`
//! subsection.

use std::fmt;

use serde::Deserialize;

/// TRMNL publishing configuration.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "mode", rename_all = "lowercase")]
#[non_exhaustive]
pub enum TrmnlConfig {
    /// Device polls our server (BYOS). v1 default.
    Byos(ByosConfig),
    /// We push images to the TRMNL cloud.
    Webhook(WebhookConfig),
}

impl TrmnlConfig {
    /// Returns the TOML tag (`"byos"` or `"webhook"`)
    /// for this variant.
    pub fn mode_name(&self) -> &'static str {
        match self {
            Self::Byos(_) => "byos",
            Self::Webhook(_) => "webhook",
        }
    }
}

impl fmt::Display for TrmnlConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.mode_name())
    }
}

/// BYOS (Bring Your Own Server) mode settings. Our
/// server returns `image_url` pointing under
/// `public_image_base`; the device fetches the BMP
/// directly.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct ByosConfig {
    /// Base URL at which rendered images are served.
    pub public_image_base: String,
    /// Default refresh rate returned in the
    /// `/api/display` response.
    #[serde(default = "default_refresh_rate_s")]
    pub default_refresh_rate_s: u32,
}

/// Cloud Webhook Image plugin settings.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct WebhookConfig {
    /// Full plugin webhook URL (includes the plugin
    /// UUID).
    pub url: String,
    /// `Content-Type` header sent with image uploads.
    #[serde(default = "default_webhook_content_type")]
    pub content_type: String,
}

fn default_refresh_rate_s() -> u32 {
    900
}

fn default_webhook_content_type() -> String {
    "image/bmp".to_owned()
}
