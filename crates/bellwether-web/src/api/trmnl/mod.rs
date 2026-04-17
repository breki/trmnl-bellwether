//! TRMNL BYOS (Bring Your Own Server) endpoints.
//!
//! The TRMNL OG firmware polls this server for its next
//! image and reports telemetry back. Endpoints:
//!
//! - `GET /api/display` — returns a JSON manifest with
//!   the `image_url` the device should fetch plus a
//!   `refresh_rate` (seconds).
//! - `POST /api/log` — accepts a small JSON telemetry
//!   blob from the device. Body limited to 16 KiB.
//!   Known fields (battery voltage, RSSI, firmware
//!   version) are logged at `INFO`; the full body is
//!   only logged at `DEBUG` to avoid flooding the
//!   journal with multi-KB lines.
//! - `GET /images/{filename}` — serves the BMP bytes.
//!
//! Images live in an in-memory [`ImageStore`] behind a
//! single composite `RwLock`, so readers of the
//! `latest_filename` pointer never see a filename whose
//! bytes aren't yet inserted. Filenames are validated
//! at insert time (`[A-Za-z0-9._-]{1,128}`) so nothing
//! user-suppliable can flow into the advertised
//! `image_url`.
//!
//! ## Authentication
//!
//! If a non-empty access token is supplied via
//! [`TrmnlState::with_access_token`] (or the
//! `BELLWETHER_ACCESS_TOKEN` env var at server
//! startup), all three endpoints require an
//! `Access-Token` request header matching it exactly.
//! Deployments that leave the server exposed beyond a
//! trusted LAN **should** set one.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::{Serialize, Serializer};

/// Maximum request body size for `POST /api/log`.
pub const MAX_LOG_BODY_BYTES: usize = 16 * 1024;

/// Max filename length accepted by
/// [`ImageStore::put_image`]. Keeps URLs short and
/// prevents accidental blowup from badly-formed
/// render-side filenames.
pub const MAX_FILENAME_LEN: usize = 128;

/// Filename validation failed.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum InvalidFilename {
    /// Empty or whitespace-only.
    #[error("filename is empty")]
    Empty,
    /// Longer than [`MAX_FILENAME_LEN`].
    #[error("filename exceeds {MAX_FILENAME_LEN} characters")]
    TooLong,
    /// Contains a character outside `A-Za-z0-9._-`.
    #[error("filename contains disallowed character {0:?}")]
    BadChar(char),
    /// Starts with `.` (reserved for hidden files).
    #[error("filename must not start with a dot")]
    LeadingDot,
}

fn validate_filename(s: &str) -> Result<(), InvalidFilename> {
    if s.is_empty() {
        return Err(InvalidFilename::Empty);
    }
    if s.len() > MAX_FILENAME_LEN {
        return Err(InvalidFilename::TooLong);
    }
    if s.starts_with('.') {
        return Err(InvalidFilename::LeadingDot);
    }
    for c in s.chars() {
        if !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
            return Err(InvalidFilename::BadChar(c));
        }
    }
    Ok(())
}

/// Base URL validation failed.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum InvalidBaseUrl {
    /// Empty after trimming slashes.
    #[error("public_image_base is empty")]
    Empty,
    /// Not `http://` or `https://`.
    #[error("public_image_base must start with http:// or https://")]
    NoScheme,
    /// Contains query string or fragment.
    #[error("public_image_base must not contain '?' or '#'")]
    UnexpectedQuery,
}

fn normalize_base(base: &str) -> Result<String, InvalidBaseUrl> {
    let trimmed = base.trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(InvalidBaseUrl::Empty);
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(InvalidBaseUrl::NoScheme);
    }
    if trimmed.contains(['?', '#']) {
        return Err(InvalidBaseUrl::UnexpectedQuery);
    }
    Ok(trimmed.to_owned())
}

/// In-memory BMP store. Composite lock so readers of
/// the "latest" pointer can never see a filename whose
/// bytes aren't in the map.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ImageStore {
    inner: RwLock<ImageStoreInner>,
}

#[derive(Debug, Default)]
struct ImageStoreInner {
    images: BTreeMap<String, Bytes>,
    latest: Option<String>,
}

impl ImageStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an image and mark it as the latest.
    /// Returns [`InvalidFilename`] if the filename
    /// doesn't match the validator.
    pub fn put_image(
        &self,
        filename: String,
        bytes: Bytes,
    ) -> Result<(), InvalidFilename> {
        validate_filename(&filename)?;
        let mut inner = self
            .inner
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.images.insert(filename.clone(), bytes);
        inner.latest = Some(filename);
        Ok(())
    }

    /// Fetch an image by filename. Returns a cheap
    /// `Bytes` handle (refcounted; no data copy).
    #[must_use]
    pub fn get_image(&self, filename: &str) -> Option<Bytes> {
        let inner = self
            .inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.images.get(filename).cloned()
    }

    /// Return the filename most recently inserted.
    #[must_use]
    pub fn latest_filename(&self) -> Option<String> {
        let inner = self
            .inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.latest.clone()
    }
}

/// Refresh interval returned to the device. The wire
/// format is `u32` seconds; the Rust type is a
/// `Duration` newtype so the unit is visible at every
/// construction site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshInterval(Duration);

impl RefreshInterval {
    /// Construct from a whole number of seconds.
    #[must_use]
    pub const fn from_secs(secs: u32) -> Self {
        Self(Duration::from_secs(secs as u64))
    }

    /// The interval as whole seconds (saturating).
    #[must_use]
    pub fn as_secs(self) -> u32 {
        u32::try_from(self.0.as_secs()).unwrap_or(u32::MAX)
    }
}

impl Serialize for RefreshInterval {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.as_secs())
    }
}

/// Axum shared state for the TRMNL BYOS routes.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TrmnlState {
    images: Arc<ImageStore>,
    public_image_base: Arc<str>,
    default_refresh_interval: RefreshInterval,
    /// If set, incoming requests must include an
    /// `Access-Token` header matching this exactly.
    access_token: Option<Arc<str>>,
}

impl TrmnlState {
    /// Build a new state. The `public_image_base` is
    /// validated (`http`/`https` scheme, no query, no
    /// trailing slash).
    pub fn new(
        public_image_base: &str,
        default_refresh_interval: RefreshInterval,
    ) -> Result<Self, InvalidBaseUrl> {
        let normalized = normalize_base(public_image_base)?;
        Ok(Self {
            images: Arc::new(ImageStore::new()),
            public_image_base: Arc::from(normalized),
            default_refresh_interval,
            access_token: None,
        })
    }

    /// Require this access token on every TRMNL
    /// endpoint. Empty strings are ignored (convenient
    /// when the token comes from an env var that may be
    /// unset).
    #[must_use]
    pub fn with_access_token(mut self, token: &str) -> Self {
        if !token.is_empty() {
            self.access_token = Some(Arc::from(token));
        }
        self
    }

    /// Store a rendered image and mark it the latest.
    pub fn put_image(
        &self,
        filename: String,
        bytes: Bytes,
    ) -> Result<(), InvalidFilename> {
        self.images.put_image(filename, bytes)
    }

    /// Fetch an image handle (refcounted, no copy).
    #[must_use]
    pub fn get_image(&self, filename: &str) -> Option<Bytes> {
        self.images.get_image(filename)
    }

    /// Filename of the most recently inserted image.
    #[must_use]
    pub fn latest_filename(&self) -> Option<String> {
        self.images.latest_filename()
    }

    /// Base URL for serving images.
    #[must_use]
    pub fn public_image_base(&self) -> &str {
        &self.public_image_base
    }

    /// Default refresh interval advertised to the
    /// device.
    #[must_use]
    pub fn default_refresh_interval(&self) -> RefreshInterval {
        self.default_refresh_interval
    }
}

/// Response body for `GET /api/display`. Matches the
/// fields the TRMNL OG firmware reads.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct DisplayResponse {
    /// Filename of the next image for the device.
    pub filename: String,
    /// Full URL to fetch the image.
    pub image_url: String,
    /// Seconds the device should sleep before the next
    /// poll.
    pub refresh_rate: RefreshInterval,
    /// Whether the device should update firmware.
    pub update_firmware: bool,
    /// URL for the firmware binary. Omitted from JSON
    /// when firmware update is not pending.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_url: Option<String>,
    /// Whether the device should soft-reset.
    pub reset_firmware: bool,
    /// Firmware-defined status code; 0 = OK.
    pub status: u16,
}

/// Handler: return the manifest for the next device
/// poll. Returns `503 Service Unavailable` when no
/// image has been rendered yet.
pub async fn display(
    State(state): State<TrmnlState>,
) -> Result<Json<DisplayResponse>, StatusCode> {
    let filename = state
        .latest_filename()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let image_url = format!("{}/{}", state.public_image_base(), filename);
    Ok(Json(DisplayResponse {
        filename,
        image_url,
        refresh_rate: state.default_refresh_interval(),
        update_firmware: false,
        firmware_url: None,
        reset_firmware: false,
        status: 0,
    }))
}

/// Handler: serve a rendered BMP. `Bytes` is
/// refcounted, so this response body is zero-copy from
/// the store. Returns `404` if the filename isn't in
/// the store — and can't be anything outside the store
/// because there's no filesystem lookup.
pub async fn serve_image(
    State(state): State<TrmnlState>,
    Path(filename): Path<String>,
) -> Response {
    match state.get_image(&filename) {
        Some(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, HeaderValue::from_static("image/bmp"))],
            bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Known fields the TRMNL OG firmware sends in its
/// telemetry blob. Unknown fields still parse (as part
/// of `extra`) and are only surfaced at the `DEBUG`
/// level.
#[derive(Debug, serde::Deserialize, Default)]
struct TelemetryPayload {
    #[serde(default)]
    battery_voltage: Option<f32>,
    #[serde(default)]
    rssi: Option<i32>,
    #[serde(default)]
    fw_version: Option<String>,
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Handler: accept a small telemetry blob. Logs known
/// fields as structured attributes at `INFO`; logs the
/// full payload at `DEBUG` for investigating unusual
/// shapes without flooding normal-level logs. Body
/// size is capped at [`MAX_LOG_BODY_BYTES`] via the
/// route's [`DefaultBodyLimit`] layer.
async fn log(Json(payload): Json<TelemetryPayload>) -> StatusCode {
    tracing::info!(
        battery_voltage = ?payload.battery_voltage,
        rssi = ?payload.rssi,
        fw_version = ?payload.fw_version,
        extra_keys = payload.extra.len(),
        "trmnl device log",
    );
    if !payload.extra.is_empty() {
        // Only at DEBUG so an unusual field won't pollute
        // normal-level logs.
        tracing::debug!(
            extra = ?payload.extra,
            "trmnl device log extras",
        );
    }
    StatusCode::NO_CONTENT
}

/// Middleware: require the configured `Access-Token`
/// header on every request. If the state has no token
/// set, requests pass through untouched.
pub async fn require_access_token(
    State(state): State<TrmnlState>,
    req: Request,
    next: Next,
) -> Response {
    let Some(expected) = state.access_token.as_ref() else {
        return next.run(req).await;
    };
    let supplied = req
        .headers()
        .get("access-token")
        .and_then(|v| v.to_str().ok());
    if supplied != Some(expected.as_ref()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(req).await
}

/// All TRMNL BYOS routes, pre-composed with the
/// [`require_access_token`] middleware. The
/// `DefaultBodyLimit` on `/api/log` is applied here so
/// callers can't forget it.
pub fn router(state: TrmnlState) -> Router {
    Router::new()
        .route("/api/display", get(display))
        .route(
            "/api/log",
            post(log).layer(DefaultBodyLimit::max(MAX_LOG_BODY_BYTES)),
        )
        .route("/images/{filename}", get(serve_image))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_access_token,
        ))
        .with_state(state)
}
