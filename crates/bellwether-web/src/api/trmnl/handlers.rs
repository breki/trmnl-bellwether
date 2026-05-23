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

/// Envelope sent by the TRMNL OG firmware to
/// `/api/log`. The device queues log entries while
/// it's in deep-sleep between wake cycles and ships
/// them all in one POST when it wakes, so `logs` is
/// typically non-empty but can hold either one entry
/// (a fresh wake) or several (catching up after a
/// long sleep / connectivity gap).
///
/// Schema sourced verbatim from
/// `usetrmnl/firmware:lib/trmnl/src/serialize_request_api_log.cpp`
/// and the per-entry shape from
/// `lib/trmnl/src/serialize_log.cpp` — every field
/// name here matches the wire JSON exactly so a
/// future firmware refresh that adds a field needs
/// only an `Option<…>` arm here, not a rename.
/// Pinned against upstream `6cf2617` (2026-05-22) at
/// the time this struct was last verified; a future
/// refactor that touches these fields should
/// re-check the firmware source at the corresponding
/// path in case the schema drifted.
#[derive(Debug, serde::Deserialize, Default)]
pub(super) struct TrmnlLogRequest {
    #[serde(default)]
    pub(super) logs: Vec<TrmnlLogEntry>,
}

/// One log entry inside [`TrmnlLogRequest`]. Field
/// names mirror the firmware's `JsonDocument` keys
/// 1:1. The device-status fields (`battery_voltage`,
/// `wifi_signal`, `firmware_version`) are snapshotted
/// at the moment the log was emitted, not at send
/// time, so the freshest reading lives in the last
/// entry of `logs`. Every field is `Option` because
/// `#[serde(default)]` lets an older firmware that
/// omits one field still parse cleanly.
///
/// Only fields the handler currently reads are typed
/// out; everything else the firmware emits
/// (`id`, `source_line`, `source_path`,
/// `wifi_status`, `sleep_duration`, `special_function`,
/// `free_heap_size`, `max_alloc_size`, `retry`, …)
/// lands in `extra` and is surfaced at `DEBUG`. Promote
/// fields out of `extra` when a real consumer appears
/// (e.g. the PR 3d "persist last device telemetry"
/// work that wants RSSI in `/api/status`).
#[derive(Debug, serde::Deserialize, Default)]
pub(super) struct TrmnlLogEntry {
    #[serde(default)]
    pub(super) created_at: Option<i64>,
    #[serde(default)]
    pub(super) message: Option<String>,
    #[serde(default)]
    pub(super) battery_voltage: Option<f32>,
    // `wifi_signal` is RSSI in dBm — small negative
    // integers in practice (-100..0). `i16` is amply
    // wide and matches the domain shape.
    #[serde(default)]
    pub(super) wifi_signal: Option<i16>,
    #[serde(default)]
    pub(super) firmware_version: Option<String>,
    // `wake_reason` is an ESP32 enum discriminator
    // (single-digit unsigned). `refresh_rate` is the
    // device's polling interval in seconds — matches
    // the `RefreshInterval: u32` newtype already in
    // this module.
    #[serde(default)]
    pub(super) wake_reason: Option<u32>,
    #[serde(default)]
    pub(super) refresh_rate: Option<u32>,
    // DEBUG-only sink: do not `.get()` from this in
    // sibling code. The right way to consume a new
    // firmware field is to promote it to a typed
    // field above, not to reach into the map. Keeping
    // this discipline means `extra` only ever holds
    // fields we haven't decided to model yet, and the
    // typed surface stays the contract.
    #[serde(flatten)]
    pub(super) extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Handler: accept a TRMNL log request, log each
/// entry as a structured event at `INFO`, log any
/// unknown fields at `DEBUG`, and cache the freshest
/// `battery_voltage` reading on `TrmnlState` so the
/// next publish tick's rendered dashboard reflects
/// current device telemetry.
///
/// "Freshest" = the last `Some` value across the
/// entries in document order. The firmware appends
/// in chronological order and the device-status
/// snapshot is taken at entry-creation time, so the
/// final entry's voltage is the most recent
/// observation. Earlier non-`None` readings still
/// get logged but don't overwrite a later reading
/// with a stale one.
///
/// Body size is capped at `MAX_LOG_BODY_BYTES` via
/// the route's `DefaultBodyLimit` layer.
pub async fn log(
    State(state): State<TrmnlState>,
    Json(payload): Json<TrmnlLogRequest>,
) -> StatusCode {
    let mut latest_voltage: Option<f32> = None;
    for entry in &payload.logs {
        tracing::info!(
            created_at = ?entry.created_at,
            message = ?entry.message,
            battery_voltage = ?entry.battery_voltage,
            wifi_signal = ?entry.wifi_signal,
            firmware_version = ?entry.firmware_version,
            wake_reason = ?entry.wake_reason,
            refresh_rate = ?entry.refresh_rate,
            extra_keys = entry.extra.len(),
            "trmnl device log",
        );
        if !entry.extra.is_empty() {
            tracing::debug!(
                extra = ?entry.extra,
                "trmnl device log extras",
            );
        }
        if entry.battery_voltage.is_some() {
            latest_voltage = entry.battery_voltage;
        }
    }
    if let Some(v) = latest_voltage {
        state.update_telemetry(DeviceTelemetry {
            battery_voltage: Some(f64::from(v)),
        });
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
