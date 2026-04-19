//! Axum request handlers for the TRMNL BYOS endpoints.
//!
//! Kept in a sibling module so `mod.rs` can focus on
//! state + router composition. Response types,
//! `FriendlyId`, and the auth middleware live here
//! next to their sole callers.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use bellwether::publish::DeviceTelemetry;
use serde::Serialize;

/// Build a 200 BMP response with the canonical
/// `Content-Type`. Kept in one place so `serve_image`
/// and `preview` can't drift if the response shape
/// grows new headers (`ETag`, `Content-Length`, etc.).
fn bmp_response(bytes: Bytes) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("image/bmp"))],
        bytes,
    )
        .into_response()
}

use super::{RefreshInterval, TrmnlState};

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

/// Six-character uppercase-hex device identifier
/// derived from a MAC. Newtype so the format invariant
/// (always six hex digits) lives in the type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FriendlyId(String);

impl FriendlyId {
    /// Derive from a device MAC. Strips non-hex
    /// characters (the device sends colon- or
    /// dash-separated MACs), takes the last six hex
    /// digits, uppercases, and left-pads with zeros if
    /// fewer than six were supplied so the ID is always
    /// a stable 6-char string.
    #[must_use]
    pub fn from_mac(mac: &str) -> Self {
        let hex: String = mac
            .chars()
            .filter(char::is_ascii_hexdigit)
            .map(|c| c.to_ascii_uppercase())
            .collect();
        let tail = hex.get(hex.len().saturating_sub(6)..).unwrap_or("");
        Self(format!("{tail:0>6}"))
    }
}

impl std::fmt::Display for FriendlyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Placeholder `api_key` returned when the operator has
/// not configured a `BELLWETHER_ACCESS_TOKEN`. The
/// value is irrelevant in that mode because the
/// [`require_access_token`] middleware is a no-op, but
/// kept as a stable named constant to make the
/// no-auth-mode contract explicit.
///
/// **Caveat**: if the operator later configures a real
/// access token, any device that previously registered
/// will send this placeholder in its `Access-Token`
/// header and be rejected. Such devices must be
/// factory-reset and re-provisioned.
pub const DEFAULT_UNCONFIGURED_API_KEY: &str = "bellwether";

/// Response body for `GET /api/setup`. Sent back to a
/// TRMNL device on first boot so it learns its
/// `api_key` (used as `Access-Token` on every later
/// request) and its `friendly_id`.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct SetupResponse {
    /// Mirrors HTTP status; the firmware inspects this
    /// field rather than the response status.
    pub status: u16,
    /// Token the device should send in its `Access-Token`
    /// header from now on.
    pub api_key: String,
    /// Short human-readable device ID.
    pub friendly_id: FriendlyId,
    /// Full URL of the first image to display.
    pub image_url: String,
    /// Filename portion of `image_url`.
    pub filename: String,
}

/// Handler: first-boot device registration. Returns the
/// `api_key` the device should use on every subsequent
/// request plus the first image to display.
///
/// Deliberately **exempt** from the
/// [`require_access_token`] middleware: a fresh device
/// has no token yet, which is the reason this endpoint
/// exists. Returns `503` when no image has been
/// rendered yet (same rationale as `/api/display`:
/// advertising a URL with nothing behind it would
/// leave the device fetching a 404).
pub async fn setup(
    State(state): State<TrmnlState>,
    headers: HeaderMap,
) -> Result<Json<SetupResponse>, StatusCode> {
    let mac = headers
        .get("id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::NOT_FOUND)?;
    let friendly_id = FriendlyId::from_mac(mac);
    let api_key = state
        .access_token()
        .map_or_else(|| DEFAULT_UNCONFIGURED_API_KEY.to_owned(), str::to_owned);
    let filename = state
        .latest_filename()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let image_url = format!("{}/{}", state.public_image_base(), filename);
    tracing::info!(
        mac = %mac,
        friendly_id = %friendly_id,
        "trmnl device setup",
    );
    Ok(Json(SetupResponse {
        status: 200,
        api_key,
        friendly_id,
        image_url,
        filename,
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
        Some(bytes) => bmp_response(bytes),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Handler: serve the most recently rendered BMP
/// without requiring an access token. Used by the
/// landing page so an operator can see the current
/// dashboard in a browser even when the TRMNL
/// endpoints are token-gated. Returns `404` when no
/// image has been rendered yet (so the `<img>`
/// onerror handler fires immediately instead of the
/// browser treating `503` as a transient error worth
/// retrying). Sets `Cache-Control: no-store` so the
/// static `/preview.bmp` URL always refreshes to the
/// latest render.
pub async fn preview(State(state): State<TrmnlState>) -> Response {
    let Some(bytes) = state.images().latest_image() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut resp = bmp_response(bytes);
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

/// Known fields the TRMNL OG firmware sends in its
/// telemetry blob. Unknown fields still parse (as part
/// of `extra`) and are only surfaced at the `DEBUG`
/// level.
#[derive(Debug, serde::Deserialize, Default)]
pub(super) struct TelemetryPayload {
    #[serde(default)]
    pub(super) battery_voltage: Option<f32>,
    #[serde(default)]
    pub(super) rssi: Option<i32>,
    #[serde(default)]
    pub(super) fw_version: Option<String>,
    #[serde(flatten)]
    pub(super) extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Handler: accept a small telemetry blob. Logs
/// known fields as structured attributes at `INFO`;
/// logs the full payload at `DEBUG` for
/// investigating unusual shapes without flooding
/// normal-level logs. Body size is capped at
/// `MAX_LOG_BODY_BYTES` via the route's
/// `DefaultBodyLimit` layer.
///
/// Also caches the parsed `battery_voltage` on
/// `TrmnlState` so the next publish tick's rendered
/// dashboard can reflect current device telemetry.
pub async fn log(
    State(state): State<TrmnlState>,
    Json(payload): Json<TelemetryPayload>,
) -> StatusCode {
    tracing::info!(
        battery_voltage = ?payload.battery_voltage,
        rssi = ?payload.rssi,
        fw_version = ?payload.fw_version,
        extra_keys = payload.extra.len(),
        "trmnl device log",
    );
    if !payload.extra.is_empty() {
        tracing::debug!(
            extra = ?payload.extra,
            "trmnl device log extras",
        );
    }
    state.update_telemetry(DeviceTelemetry {
        battery_voltage: payload.battery_voltage.map(f64::from),
    });
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
    let Some(expected) = state.access_token() else {
        return next.run(req).await;
    };
    let supplied = req
        .headers()
        .get("access-token")
        .and_then(|v| v.to_str().ok());
    if supplied != Some(expected) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(req).await
}
