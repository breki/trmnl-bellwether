//! Windy Point Forecast v2 client.
//!
//! Docs: <https://api.windy.com/point-forecast/docs>.
//!
//! The client posts `lat`/`lon`/`model`/`parameters`/`key`
//! to Windy and returns a [`Forecast`] with parsed
//! timestamps and named value series. Unit conversion
//! (e.g., Kelvin → Celsius, u/v → speed) is left to the
//! renderer — this module is a thin transport layer.
//!
//! ## Security posture
//!
//! - API key is `POST`ed in the JSON body (Windy's
//!   convention, not a header). It is **redacted** from
//!   error bodies before they surface in [`WindyError`].
//! - Redirects are disabled: a compromised DNS entry
//!   for `api.windy.com` redirecting to an attacker
//!   would otherwise have the body (including the key)
//!   re-POSTed by reqwest. See [`Policy::none`].
//! - Response bodies are read with a byte cap
//!   ([`MAX_RESPONSE_BYTES`]) to prevent OOM from a
//!   misbehaving proxy or server.
//! - `null` values in series are preserved as
//!   `Option<f64>` — Windy returns `null` at grid edges
//!   for some models.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};

use crate::config::{WindyConfig, WindyParameter};

#[cfg(test)]
mod tests;

/// Default Windy API host.
pub const DEFAULT_BASE_URL: &str = "https://api.windy.com";

/// Relative path of the Point Forecast v2 endpoint.
pub const ENDPOINT_PATH: &str = "/api/point-forecast/v2";

/// Default full endpoint URL (host + path).
pub const DEFAULT_ENDPOINT: &str =
    "https://api.windy.com/api/point-forecast/v2";

/// Cap for successful-response body size. Windy responses
/// are typically 10–200 KB; 4 MiB is a comfortable
/// ceiling that still prevents OOM from a runaway proxy.
pub const MAX_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;

/// Cap for error-response body size. Error bodies are
/// diagnostic-only; keep them tight.
pub const MAX_ERROR_BODY_BYTES: u64 = 4096;

/// Maximum characters kept in the [`WindyError::Api`]
/// body before truncation.
pub const ERROR_BODY_MESSAGE_CAP: usize = 512;

/// Request parameters for a single forecast fetch.
///
/// All fields are owned so schedulers can store a
/// `FetchRequest` between ticks without lifetime
/// juggling. The `Debug` impl redacts the API key so
/// the struct is safe to `tracing::debug!(?req, …)`.
#[derive(Clone)]
pub struct FetchRequest {
    /// Windy API key.
    pub api_key: String,
    /// Point latitude in decimal degrees.
    pub lat: f64,
    /// Point longitude in decimal degrees.
    pub lon: f64,
    /// Forecast model (e.g., `"gfs"`).
    pub model: String,
    /// Parameters to request. Must be non-empty;
    /// [`Client::fetch`] returns [`WindyError::NoParameters`]
    /// if empty.
    pub parameters: Vec<WindyParameter>,
}

impl std::fmt::Debug for FetchRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchRequest")
            .field("api_key", &"<redacted>")
            .field("lat", &self.lat)
            .field("lon", &self.lon)
            .field("model", &self.model)
            .field("parameters", &self.parameters)
            .finish()
    }
}

impl FetchRequest {
    /// Build a [`FetchRequest`] from a loaded
    /// [`WindyConfig`]. Returns
    /// [`WindyError::MissingApiKey`] if the config was
    /// parsed via [`Config::from_toml_str`](crate::config::Config::from_toml_str)
    /// (which does not populate the key).
    pub fn from_config(cfg: &WindyConfig) -> Result<Self, WindyError> {
        let api_key =
            cfg.api_key().ok_or(WindyError::MissingApiKey)?.to_owned();
        Ok(Self {
            api_key,
            lat: cfg.lat,
            lon: cfg.lon,
            model: cfg.model.clone(),
            parameters: cfg.parameters.clone(),
        })
    }
}

/// A parsed Windy forecast response.
///
/// Series keys follow Windy's wire convention
/// (`"{param}-{level}"`, e.g. `"temp-surface"`,
/// `"wind_u-surface"`). Prefer [`Forecast::values`] for
/// a typed lookup.
///
/// Non-numeric fields Windy may return alongside series
/// (e.g., forward-compat metadata) are silently ignored
/// rather than causing a parse failure.
#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    /// UTC timestamps for each forecast step. Guaranteed
    /// non-empty — Windy returning an empty `ts` array
    /// produces [`WindyError::EmptyForecast`].
    pub timestamps: Vec<DateTime<Utc>>,
    /// Units for each series key, as Windy reported them.
    pub units: HashMap<String, String>,
    /// Numeric series, keyed by `"{param}-{level}"`.
    /// Every series has the same length as
    /// [`Forecast::timestamps`] (enforced at parse time).
    /// `None` entries are Windy's `null` markers.
    pub series: HashMap<String, Vec<Option<f64>>>,
    /// Windy's rate-limit or data-quality warning.
    pub warning: Option<String>,
}

impl Forecast {
    /// Surface-level values for a given parameter.
    /// Returns `None` if Windy didn't include that
    /// parameter in the response.
    pub fn values(&self, parameter: WindyParameter) -> Option<&[Option<f64>]> {
        let key = format!("{}-surface", parameter.wire_name());
        self.series.get(&key).map(Vec::as_slice)
    }

    /// Build a `Forecast` from a raw Windy JSON string.
    /// Exposed for tests that want to construct a
    /// `Forecast` without an HTTP round-trip — this runs
    /// the same parsing + length-checking logic the
    /// live client uses, so fixtures can't silently
    /// drift out of the wire contract.
    pub fn from_raw_json(json: &str) -> Result<Self, WindyError> {
        let raw: RawResponse = serde_json::from_str(json)
            .map_err(|e| WindyError::Parse(e.to_string()))?;
        raw.into_forecast()
    }
}

/// Errors returned by [`Client::fetch`].
///
/// `Http` wraps transport, TLS, decode, and client-build
/// failures from `reqwest`. Callers that need to
/// distinguish (e.g., retry on timeout) can match on the
/// inner error via `.is_timeout()` / `.is_connect()`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WindyError {
    /// Transport / TLS / decode / client-build failure.
    #[error("Windy HTTP error: {0}")]
    Http(Box<reqwest::Error>),
    /// Windy returned a non-2xx response. The body is
    /// redacted of the API key and truncated to at most
    /// [`ERROR_BODY_MESSAGE_CAP`] characters.
    #[error("Windy API returned status {status}: {body}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Truncated, key-redacted response body.
        body: Box<str>,
    },
    /// Response body exceeded [`MAX_RESPONSE_BYTES`] /
    /// [`MAX_ERROR_BODY_BYTES`].
    #[error("Windy response body exceeded {limit} bytes")]
    ResponseTooLarge {
        /// The cap that was exceeded.
        limit: u64,
    },
    /// Response JSON was malformed or shape-incompatible.
    /// Wraps a `serde_json` message (owned String so the
    /// `serde_json` type doesn't leak into our public
    /// source chain).
    #[error("parsing Windy response: {0}")]
    Parse(String),
    /// Windy returned a timestamp outside the
    /// `DateTime<Utc>` range. `ms` is the raw value.
    #[error(
        "invalid timestamp {ms} ms from Windy response \
         (outside DateTime<Utc> range)"
    )]
    InvalidTimestamp {
        /// Raw epoch-milliseconds value.
        ms: i64,
    },
    /// A numeric series had a different length than
    /// `ts`. Indicates wire-format drift.
    #[error("series `{key}` has length {got} but ts has length {expected}")]
    SeriesLengthMismatch {
        /// Series key.
        key: String,
        /// Expected length (equal to `ts.len()`).
        expected: usize,
        /// Actual length.
        got: usize,
    },
    /// Windy returned `ts: []`. Treated as an error
    /// because every downstream consumer assumes at
    /// least one step.
    #[error("Windy returned an empty forecast (ts is empty)")]
    EmptyForecast,
    /// Caller passed a [`FetchRequest`] with no
    /// parameters. Windy would reject it anyway; we
    /// short-circuit to avoid a wasted call.
    #[error("cannot fetch a forecast with no parameters")]
    NoParameters,
    /// The [`WindyConfig`] handed to
    /// [`FetchRequest::from_config`] had `api_key()
    /// == None`. This means the config came from
    /// [`Config::from_toml_str`](crate::config::Config::from_toml_str)
    /// rather than [`Config::load`](crate::config::Config::load).
    #[error(
        "Windy API key is not loaded; construct the config \
         via Config::load to populate it"
    )]
    MissingApiKey,
}

impl From<reqwest::Error> for WindyError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(Box::new(e))
    }
}

/// HTTP client for the Windy Point Forecast API.
///
/// Build once per process and reuse. Construction pays
/// for TLS setup and connection-pool initialization;
/// `Client` wraps a `reqwest::Client` which is cheap to
/// clone (refcounted internally).
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    max_response_bytes: u64,
    max_error_body_bytes: u64,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Build a client pointing at the public Windy API.
    ///
    /// Panics only if the OS's TLS init fails, which is
    /// a process-wide condition — there's no meaningful
    /// recovery, and every call site would `.unwrap()`
    /// anyway. Matches `reqwest::Client::new`'s
    /// semantics.
    #[must_use]
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    /// Build a client pointing at a custom base URL
    /// (useful for tests against `wiremock` and for
    /// local proxies).
    #[must_use]
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("bellwether/", env!("CARGO_PKG_VERSION"),))
            .redirect(Policy::none())
            .build()
            .expect("reqwest builder with rustls-tls is infallible");
        Self {
            http,
            base_url: base_url.into(),
            max_response_bytes: MAX_RESPONSE_BYTES,
            max_error_body_bytes: MAX_ERROR_BODY_BYTES,
        }
    }

    /// Override the success-response size cap. Useful
    /// for tests that want to trigger
    /// [`WindyError::ResponseTooLarge`] without
    /// generating a multi-megabyte body.
    #[must_use]
    pub fn with_max_response_bytes(mut self, limit: u64) -> Self {
        self.max_response_bytes = limit;
        self
    }

    /// Override the error-response size cap.
    #[must_use]
    pub fn with_max_error_body_bytes(mut self, limit: u64) -> Self {
        self.max_error_body_bytes = limit;
        self
    }

    /// Full endpoint URL for this client.
    #[must_use]
    pub fn endpoint(&self) -> String {
        format!("{}{}", self.base_url, ENDPOINT_PATH)
    }

    /// Fetch a forecast using the lat/lon/model/params
    /// from a loaded [`WindyConfig`]. Convenience for
    /// the common path.
    pub async fn fetch_with_config(
        &self,
        cfg: &WindyConfig,
    ) -> Result<Forecast, WindyError> {
        let req = FetchRequest::from_config(cfg)?;
        self.fetch(&req).await
    }

    /// Fetch a forecast. Takes the request by reference
    /// so callers (like the publish loop) can keep a
    /// cached copy without cloning its `String`s per
    /// tick.
    pub async fn fetch(
        &self,
        req: &FetchRequest,
    ) -> Result<Forecast, WindyError> {
        if req.parameters.is_empty() {
            return Err(WindyError::NoParameters);
        }
        let body = RequestBody {
            lat: req.lat,
            lon: req.lon,
            model: &req.model,
            parameters: &req.parameters,
            key: &req.api_key,
            levels: &["surface"],
        };
        let resp = self.http.post(self.endpoint()).json(&body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let raw = read_capped_body(resp, self.max_error_body_bytes).await?;
            let redacted = redact_secret(&raw, &req.api_key);
            let truncated = truncate(redacted, ERROR_BODY_MESSAGE_CAP);
            return Err(WindyError::Api {
                status: status.as_u16(),
                body: truncated.into_boxed_str(),
            });
        }
        let raw_text = read_capped_body(resp, self.max_response_bytes).await?;
        let raw: RawResponse = serde_json::from_str(&raw_text)
            .map_err(|e| WindyError::Parse(e.to_string()))?;
        raw.into_forecast()
    }
}

#[derive(Serialize)]
struct RequestBody<'a> {
    lat: f64,
    lon: f64,
    model: &'a str,
    parameters: &'a [WindyParameter],
    key: &'a str,
    levels: &'a [&'static str],
}

#[derive(Deserialize)]
struct RawResponse {
    ts: Vec<i64>,
    #[serde(default)]
    units: HashMap<String, String>,
    #[serde(default)]
    warning: Option<String>,
    /// Captures every non-reserved top-level field as a
    /// JSON `Value`. Only entries that successfully
    /// deserialize as `Vec<Option<f64>>` become series;
    /// everything else is forward-compat noise we ignore.
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

impl RawResponse {
    fn into_forecast(self) -> Result<Forecast, WindyError> {
        if self.ts.is_empty() {
            return Err(WindyError::EmptyForecast);
        }
        let timestamps: Vec<DateTime<Utc>> = self
            .ts
            .iter()
            .map(|&ms| {
                Utc.timestamp_millis_opt(ms)
                    .single()
                    .ok_or(WindyError::InvalidTimestamp { ms })
            })
            .collect::<Result<_, _>>()?;

        let mut series: HashMap<String, Vec<Option<f64>>> = HashMap::new();
        for (key, value) in self.extra {
            // Only accept entries that look like a series
            // of nullable numbers. Unknown shapes are
            // forward-compat fields; ignore them.
            let Ok(v) = serde_json::from_value::<Vec<Option<f64>>>(value)
            else {
                continue;
            };
            if v.len() != timestamps.len() {
                return Err(WindyError::SeriesLengthMismatch {
                    key,
                    expected: timestamps.len(),
                    got: v.len(),
                });
            }
            series.insert(key, v);
        }

        Ok(Forecast {
            timestamps,
            units: self.units,
            series,
            warning: self.warning,
        })
    }
}

async fn read_capped_body(
    mut resp: reqwest::Response,
    limit: u64,
) -> Result<String, WindyError> {
    if let Some(len) = resp.content_length()
        && len > limit
    {
        return Err(WindyError::ResponseTooLarge { limit });
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut total: u64 = 0;
    while let Some(chunk) = resp.chunk().await? {
        total = total.saturating_add(chunk.len() as u64);
        if total > limit {
            return Err(WindyError::ResponseTooLarge { limit });
        }
        buf.extend_from_slice(&chunk);
    }
    String::from_utf8(buf).map_err(|e| {
        WindyError::Parse(format!("response was not valid UTF-8: {e}"))
    })
}

/// Replace every literal occurrence of `secret` in `text`
/// with `"<redacted>"`. Empty `secret` is a no-op so an
/// unset `api_key` doesn't blank the whole body.
fn redact_secret(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        return text.to_owned();
    }
    text.replace(secret, "<redacted>")
}

fn truncate(mut s: String, max_len: usize) -> String {
    if s.len() <= max_len {
        return s;
    }
    // Truncate at a valid UTF-8 boundary.
    let mut cut = max_len;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
    s.push_str("…(truncated)");
    s
}
