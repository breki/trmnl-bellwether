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

mod handlers;
#[cfg(test)]
mod tests;

#[cfg(test)]
use handlers::{DEFAULT_UNCONFIGURED_API_KEY, FriendlyId};

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::{get, post};
use bellwether::publish::{DeviceTelemetry, ImageSink, SinkError};
use serde::{Serialize, Serializer};

use handlers::{
    display, log, preview, require_access_token, serve_image, setup,
};

/// Maximum request body size for `POST /api/log`.
pub const MAX_LOG_BODY_BYTES: usize = 16 * 1024;

/// Max filename length accepted by
/// [`ImageStore::put_image`]. Keeps URLs short and
/// prevents accidental blowup from badly-formed
/// render-side filenames.
pub const MAX_FILENAME_LEN: usize = 128;

/// How many rendered images the in-memory store keeps
/// before evicting the oldest. Sized generously relative
/// to the protocol requirement (which only needs the
/// image most recently advertised via `/api/display`):
/// keeping a small tail covers devices that fetch
/// slightly after the next render tick, without letting
/// memory grow unbounded — ~48 KB per 1-bit 800×480 BMP
/// × 4 = ~192 KB, bounded regardless of uptime.
pub const MAX_RETAINED_IMAGES: usize = 4;

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

    /// Insert an image, mark it as the latest, and
    /// evict the oldest entries until at most
    /// [`MAX_RETAINED_IMAGES`] remain. The current
    /// `latest` is never evicted, so a non-monotonic
    /// filename arriving last won't get pruned by
    /// lexical-order sweep.
    ///
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
        while inner.images.len() > MAX_RETAINED_IMAGES {
            let to_remove = inner
                .images
                .keys()
                .find(|k| Some(k.as_str()) != inner.latest.as_deref())
                .cloned();
            match to_remove {
                Some(k) => {
                    inner.images.remove(&k);
                }
                None => break,
            }
        }
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

    /// Atomically return the bytes of the latest image.
    /// Takes the read lock once so readers can't observe
    /// a `latest` pointer whose bytes are absent — even
    /// though [`put_image`] preserves `latest` across
    /// eviction today, keeping the lookup inside a
    /// single lock makes the invariant local to the
    /// store instead of something handlers have to
    /// reason about.
    #[must_use]
    pub fn latest_image(&self) -> Option<Bytes> {
        let inner = self
            .inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let filename = inner.latest.as_deref()?;
        inner.images.get(filename).cloned()
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

    /// The interval as a `Duration`.
    #[must_use]
    pub fn as_duration(self) -> Duration {
        self.0
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
    /// Latest telemetry parsed from a `/api/log`
    /// post. `RwLock` for consistency with the
    /// module's other caches (`ImageStore`); reads
    /// copy out a snapshot so the lock is held only
    /// briefly, writes merge under an exclusive lock.
    telemetry: Arc<RwLock<DeviceTelemetry>>,
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
            telemetry: Arc::new(RwLock::new(DeviceTelemetry::default())),
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

    /// Borrow the underlying [`ImageStore`]. Lets
    /// handlers call the store's atomic accessors (e.g.
    /// [`ImageStore::latest_image`]) without forcing
    /// every such accessor to be mirrored as a thin
    /// delegating method on `TrmnlState`.
    #[must_use]
    pub fn images(&self) -> &ImageStore {
        &self.images
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

    /// Configured access token, if any. Returns `None`
    /// when no token is required (LAN deployments where
    /// the operator leaves `BELLWETHER_ACCESS_TOKEN`
    /// unset).
    #[must_use]
    pub fn access_token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }

    /// Merge fresh fields into the cached device
    /// telemetry. Called by the `/api/log` handler
    /// every time the device posts. Fields in `update`
    /// that are `None` leave the previous cached
    /// value intact — posts without a battery voltage
    /// (keepalives, error reports) don't wipe the
    /// most recent battery reading.
    pub fn update_telemetry(&self, update: DeviceTelemetry) {
        let mut cached = self
            .telemetry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cached.merge_from(update);
    }

    /// Snapshot the cached device telemetry. Used by
    /// both the `ImageSink` impl (read by the publish
    /// loop each tick) and by tests.
    #[must_use]
    pub fn telemetry(&self) -> DeviceTelemetry {
        *self
            .telemetry
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl ImageSink for TrmnlState {
    fn publish_image(
        &self,
        filename: String,
        bytes: Vec<u8>,
    ) -> Result<(), SinkError> {
        self.put_image(filename, Bytes::from(bytes))
            .map_err(|e| Box::new(e) as SinkError)
    }

    fn latest_telemetry(&self) -> DeviceTelemetry {
        self.telemetry()
    }
}

/// All TRMNL BYOS routes, pre-composed with the
/// [`require_access_token`] middleware. The
/// `DefaultBodyLimit` on `/api/log` is applied here so
/// callers can't forget it.
pub fn router(state: TrmnlState) -> Router {
    // Routes added BEFORE `route_layer` are wrapped by
    // `require_access_token`. `/api/setup` is added
    // AFTER the layer so it remains reachable by a
    // fresh device that doesn't yet have an api_key.
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
        .route("/api/setup", get(setup))
        .route("/preview.bmp", get(preview))
        .with_state(state)
}
