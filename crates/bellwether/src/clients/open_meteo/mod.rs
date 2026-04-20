//! Open-Meteo Forecast API v1 client.
//!
//! Docs: <https://open-meteo.com/en/docs>.
//!
//! Open-Meteo is free, keyless, and already returns
//! the units the dashboard wants (°C, km/h, mm,
//! percent, compass degrees). This module is a
//! thin transport layer: HTTP GET, parse JSON,
//! map fields to [`WeatherSnapshot`].
//!
//! ## Security posture
//!
//! - No API key to protect — the endpoint is
//!   publicly readable by design.
//! - Redirects are bounded (3 hops) so a CDN
//!   canonical-host bounce works but a misconfigured
//!   upstream can't run away.
//! - Response bodies are read with a byte cap
//!   ([`MAX_RESPONSE_BYTES`]) to prevent OOM from a
//!   misbehaving proxy or server.
//! - `null` values in series are preserved as
//!   `Option<f64>` — Open-Meteo may return them at
//!   the start or end of the requested horizon.
//! - Non-finite numeric values in the response are
//!   coerced to `None` before they enter the
//!   snapshot — a provider sentinel like `-9999` or
//!   a NaN leak cannot poison `feels_like_c` or
//!   daily-high calculations downstream.

use std::error::Error as StdError;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;

use super::http_util::{self, ReadBodyError};
use crate::config::OpenMeteoProviderConfig;
use crate::dashboard::astro::GeoPoint;
use crate::dashboard::classify::{WeatherCode, WmoCode};
use crate::weather::{
    WeatherError, WeatherProvider, WeatherSnapshot, WeatherSnapshotBuilder,
};

#[cfg(test)]
mod tests;

/// Default Open-Meteo API host.
pub const DEFAULT_BASE_URL: &str = "https://api.open-meteo.com";

/// Relative path of the forecast endpoint.
pub const ENDPOINT_PATH: &str = "/v1/forecast";

/// Cap for successful-response body size. Hourly
/// forecasts for 4 days are typically 5–30 KB; 4 MiB
/// is a comfortable ceiling.
pub const MAX_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;

/// Cap for error-response body size.
pub const MAX_ERROR_BODY_BYTES: u64 = 4096;

/// Maximum characters kept in the
/// [`OpenMeteoError::Api`] body before truncation.
pub const ERROR_BODY_MESSAGE_CAP: usize = 512;

/// How many days of hourly forecast to request. The
/// dashboard needs today + 3 future days, so 4 is
/// enough.
pub const FORECAST_DAYS: u32 = 4;

/// Decimal places for lat/lon in the query string.
/// `f64::to_string` can emit scientific notation for
/// subnormals which Open-Meteo's parser rejects;
/// 6 decimals ≈ 11 cm precision, comfortably below
/// any grid resolution.
const LATLON_DECIMALS: usize = 6;

/// Hourly variables the client always requests.
/// Snake-case names match the modern Open-Meteo API.
const HOURLY_VARIABLES: &str = "temperature_2m,relative_humidity_2m,precipitation,\
     cloud_cover,wind_speed_10m,wind_direction_10m,wind_gusts_10m,\
     weather_code";

/// Request parameters for a single forecast fetch.
///
/// Construct via [`FetchRequest::from_parts`] given
/// the point of interest and the validated
/// [`OpenMeteoProviderConfig`]. The library no
/// longer exposes a `from_config(&WeatherConfig)`
/// helper: config-shape validation is the
/// [`Config::load`](crate::config::Config::load)
/// layer's job, so every provider construction in
/// this module is infallible once the caller has
/// the right subtable in hand.
#[derive(Debug, Clone)]
pub struct FetchRequest {
    /// Point latitude in decimal degrees.
    pub lat: f64,
    /// Point longitude in decimal degrees.
    pub lon: f64,
    /// Open-Meteo model name (e.g. `"icon_eu"`,
    /// `"best_match"`, `"gfs_global"`).
    pub model: String,
}

impl FetchRequest {
    /// Build from validated pieces — the caller has
    /// already confirmed `[weather.open_meteo]` was
    /// present during config load.
    #[must_use]
    pub fn from_parts(
        lat: f64,
        lon: f64,
        sub: &OpenMeteoProviderConfig,
    ) -> Self {
        Self {
            lat,
            lon,
            model: sub.model.clone(),
        }
    }
}

/// Errors returned by [`Client::fetch`]. Only
/// transport / wire-level failures appear here;
/// configuration-shape errors belong to
/// [`ConfigError`](crate::config::ConfigError).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OpenMeteoError {
    /// Transport / TLS / decode / client-build
    /// failure. The inner error is boxed as a
    /// `dyn StdError` so the public enum doesn't
    /// leak the `reqwest` version.
    #[error("Open-Meteo HTTP error: {0}")]
    Http(Box<dyn StdError + Send + Sync>),
    /// Open-Meteo returned a non-2xx response. The
    /// body is truncated to
    /// [`ERROR_BODY_MESSAGE_CAP`] characters.
    #[error("Open-Meteo API returned status {status}: {body}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Truncated response body.
        body: Box<str>,
    },
    /// Response body exceeded [`MAX_RESPONSE_BYTES`].
    #[error("Open-Meteo response body exceeded {limit} bytes")]
    ResponseTooLarge {
        /// The cap that was exceeded.
        limit: u64,
    },
    /// Response JSON was malformed. Preserves the
    /// underlying `serde_json::Error` so the
    /// diagnostic carries line/column info.
    #[error("parsing Open-Meteo response: {0}")]
    Json(#[from] serde_json::Error),
    /// Response body wasn't valid UTF-8.
    #[error("Open-Meteo response was not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    /// A `time` entry couldn't be parsed as a
    /// naive UTC ISO-8601 string.
    #[error("invalid timestamp {raw:?} in Open-Meteo response")]
    InvalidTimestamp {
        /// Raw string Open-Meteo returned.
        raw: String,
    },
    /// A series had a different length than `time`.
    /// Open-Meteo documents that every hourly array
    /// matches `time.len()`, so this is wire drift
    /// worth surfacing as an error rather than
    /// silently padding.
    #[error(
        "Open-Meteo series `{series}` has length \
         {got} but `time` has length {expected}"
    )]
    SeriesLengthMismatch {
        /// Field name of the mismatched series.
        series: &'static str,
        /// `time.len()`.
        expected: usize,
        /// Actual series length.
        got: usize,
    },
    /// [`WeatherSnapshotBuilder::build`] rejected the
    /// parsed response — catches invariant bugs in
    /// the parser itself.
    #[error("Open-Meteo snapshot invariant: {0}")]
    Invariant(WeatherError),
}

impl From<reqwest::Error> for OpenMeteoError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(Box::new(e))
    }
}

impl From<ReadBodyError> for OpenMeteoError {
    fn from(e: ReadBodyError) -> Self {
        match e {
            ReadBodyError::Transport(inner) => Self::Http(Box::new(inner)),
            ReadBodyError::TooLarge { limit } => {
                Self::ResponseTooLarge { limit }
            }
            ReadBodyError::NotUtf8(inner) => Self::Utf8(inner),
        }
    }
}

impl From<OpenMeteoError> for WeatherError {
    fn from(e: OpenMeteoError) -> Self {
        match e {
            OpenMeteoError::Http(inner) => WeatherError::Transport(inner),
            other => WeatherError::Provider(Box::new(other)),
        }
    }
}

/// HTTP client for the Open-Meteo forecast API.
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
    /// Build a client pointing at the public
    /// Open-Meteo API.
    #[must_use]
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    /// Build a client pointing at a custom base URL
    /// (useful for `wiremock` tests).
    #[must_use]
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            http: http_util::build_http_client(),
            base_url: base_url.into(),
            max_response_bytes: MAX_RESPONSE_BYTES,
            max_error_body_bytes: MAX_ERROR_BODY_BYTES,
        }
    }

    /// Override the success-response size cap.
    #[must_use]
    pub fn with_max_response_bytes(mut self, limit: u64) -> Self {
        self.max_response_bytes = limit;
        self
    }

    /// Full endpoint URL for this client.
    #[must_use]
    pub fn endpoint(&self) -> String {
        format!("{}{}", self.base_url, ENDPOINT_PATH)
    }

    /// Fetch a forecast and normalise it into a
    /// [`WeatherSnapshot`].
    pub async fn fetch(
        &self,
        req: &FetchRequest,
    ) -> Result<WeatherSnapshot, OpenMeteoError> {
        let forecast_days = FORECAST_DAYS.to_string();
        // Fixed decimal format to avoid scientific
        // notation for subnormals — Open-Meteo's
        // query parser rejects `"2.2e-308"`.
        let lat = format!("{:.*}", LATLON_DECIMALS, req.lat);
        let lon = format!("{:.*}", LATLON_DECIMALS, req.lon);
        // `timezone=utc` is explicitly rejected by the
        // API; the default (GMT) matches UTC and keeps
        // the returned `time` strings parseable as
        // naive UTC in `RawResponse::into_snapshot`.
        let query = [
            ("latitude", lat.as_str()),
            ("longitude", lon.as_str()),
            ("hourly", HOURLY_VARIABLES),
            ("forecast_days", forecast_days.as_str()),
            ("models", req.model.as_str()),
        ];
        let resp = self.http.get(self.endpoint()).query(&query).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body =
                http_util::read_capped_body(resp, self.max_error_body_bytes)
                    .await?;
            let truncated =
                http_util::truncate_with_ellipsis(body, ERROR_BODY_MESSAGE_CAP);
            return Err(OpenMeteoError::Api {
                status: status.as_u16(),
                body: truncated.into_boxed_str(),
            });
        }
        let raw_text =
            http_util::read_capped_body(resp, self.max_response_bytes).await?;
        let raw: RawResponse = serde_json::from_str(&raw_text)?;
        raw.into_snapshot()
    }
}

#[derive(Deserialize)]
struct RawResponse {
    hourly: RawHourly,
}

#[derive(Deserialize)]
struct RawHourly {
    time: Vec<String>,
    #[serde(default)]
    temperature_2m: Option<Vec<Option<f64>>>,
    #[serde(default)]
    relative_humidity_2m: Option<Vec<Option<f64>>>,
    #[serde(default)]
    precipitation: Option<Vec<Option<f64>>>,
    #[serde(default)]
    cloud_cover: Option<Vec<Option<f64>>>,
    #[serde(default)]
    wind_speed_10m: Option<Vec<Option<f64>>>,
    #[serde(default)]
    wind_direction_10m: Option<Vec<Option<f64>>>,
    #[serde(default)]
    wind_gusts_10m: Option<Vec<Option<f64>>>,
    #[serde(default)]
    weather_code: Option<Vec<Option<f64>>>,
}

impl RawResponse {
    fn into_snapshot(self) -> Result<WeatherSnapshot, OpenMeteoError> {
        let h = self.hourly;
        let timestamps: Vec<DateTime<Utc>> = h
            .time
            .iter()
            .map(|raw| {
                NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M")
                    .map(|dt| dt.and_utc())
                    .map_err(|_| OpenMeteoError::InvalidTimestamp {
                        raw: raw.clone(),
                    })
            })
            .collect::<Result<_, _>>()?;
        let n = timestamps.len();
        let builder = WeatherSnapshotBuilder {
            timestamps,
            temperature_c: pick_series("temperature_c", h.temperature_2m, n)?,
            humidity_pct: pick_series(
                "humidity_pct",
                h.relative_humidity_2m,
                n,
            )?,
            wind_kmh: pick_series("wind_kmh", h.wind_speed_10m, n)?,
            wind_dir_deg: pick_series("wind_dir_deg", h.wind_direction_10m, n)?,
            gust_kmh: pick_series("gust_kmh", h.wind_gusts_10m, n)?,
            cloud_cover_pct: pick_series("cloud_cover_pct", h.cloud_cover, n)?,
            precip_mm: pick_series("precip_mm", h.precipitation, n)?,
            weather_code: pick_weather_code_series(h.weather_code, n)?,
            warning: None,
        };
        builder.build().map_err(OpenMeteoError::Invariant)
    }
}

/// Return the series if Open-Meteo sent one (with
/// non-finite entries coerced to `None`), or a
/// `Vec<None>` of length `n` if the field was
/// absent. A length mismatch surfaces as a typed
/// [`OpenMeteoError::SeriesLengthMismatch`] so
/// silent half-a-forecast bugs can't slip through.
fn pick_series(
    name: &'static str,
    series: Option<Vec<Option<f64>>>,
    n: usize,
) -> Result<Vec<Option<f64>>, OpenMeteoError> {
    match series {
        Some(s) if s.len() == n => Ok(sanitise_non_finite(s)),
        Some(s) => Err(OpenMeteoError::SeriesLengthMismatch {
            series: name,
            expected: n,
            got: s.len(),
        }),
        None => Ok(vec![None; n]),
    }
}

/// Narrow the raw JSON numeric series for
/// `weather_code` into `Vec<Option<WeatherCode>>`.
///
/// Three outcomes per hour:
/// - Non-integer / non-finite / negative / >255 →
///   `None` (the provider's contract is "integer
///   weather code in the u8 range", so anything else
///   is wire noise).
/// - Integer in `0..=255` that's outside the
///   documented WMO 4677 subset →
///   `Some(WeatherCode::Unrecognised(byte))` so the
///   display can surface
///   [`ConditionCategory::Unknown`](crate::dashboard::classify::ConditionCategory::Unknown)
///   instead of silently heuristic-guessing.
/// - Integer matching a documented variant →
///   `Some(WeatherCode::Wmo(code))`.
///
/// An absent series yields `vec![None; n]` like
/// [`pick_series`]; a length mismatch surfaces as
/// [`OpenMeteoError::SeriesLengthMismatch`].
fn pick_weather_code_series(
    series: Option<Vec<Option<f64>>>,
    n: usize,
) -> Result<Vec<Option<WeatherCode>>, OpenMeteoError> {
    match series {
        Some(s) if s.len() == n => {
            Ok(s.into_iter()
                .map(|v| {
                    let x = v?;
                    if !x.is_finite() || x.fract() != 0.0 {
                        return None;
                    }
                    // Narrow to u8 first; anything outside
                    // this range isn't a weather code at
                    // all (negative / absurdly large /
                    // fractional). `WmoCode::try_from`
                    // then sorts in-range bytes into
                    // recognised vs. unrecognised.
                    if !(0.0..=255.0).contains(&x) {
                        return None;
                    }
                    // Safe: range-checked non-negative
                    // integer ≤ 255; no truncation or sign
                    // loss possible.
                    #[allow(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss
                    )]
                    let raw = x as u8;
                    Some(WmoCode::try_from(raw).map_or(
                        WeatherCode::Unrecognised(raw),
                        WeatherCode::Wmo,
                    ))
                })
                .collect())
        }
        Some(s) => Err(OpenMeteoError::SeriesLengthMismatch {
            series: "weather_code",
            expected: n,
            got: s.len(),
        }),
        None => Ok(vec![None; n]),
    }
}

/// Map NaN / ±Inf entries to `None`. A provider
/// sentinel like `-9999` remains as `Some(-9999.0)`
/// — only truly non-finite IEEE-754 values are
/// treated as absent.
fn sanitise_non_finite(series: Vec<Option<f64>>) -> Vec<Option<f64>> {
    series
        .into_iter()
        .map(|v| v.filter(|x| x.is_finite()))
        .collect()
}

/// [`WeatherProvider`] backed by Open-Meteo.
#[derive(Debug, Clone)]
pub struct OpenMeteoProvider {
    client: Client,
    request: FetchRequest,
}

impl OpenMeteoProvider {
    /// Build from a prebuilt client and request.
    #[must_use]
    pub fn new(client: Client, request: FetchRequest) -> Self {
        Self { client, request }
    }
}

#[async_trait]
impl WeatherProvider for OpenMeteoProvider {
    fn location(&self) -> GeoPoint {
        GeoPoint {
            lat_deg: self.request.lat,
            lon_deg: self.request.lon,
        }
    }

    async fn fetch(&self) -> Result<WeatherSnapshot, WeatherError> {
        let snapshot = self.client.fetch(&self.request).await?;
        Ok(snapshot)
    }
}
