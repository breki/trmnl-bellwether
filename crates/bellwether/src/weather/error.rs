//! Errors returned by [`WeatherProvider::fetch`]
//! and by snapshot validation.
//!
//! [`WeatherProvider::fetch`]: super::WeatherProvider::fetch

use std::error::Error as StdError;

/// Boxed error alias for transport / provider
/// escape-hatch variants. Providers can convert
/// their own crate-specific errors into this form
/// without leaking dependency types into the neutral
/// `weather` module.
pub type BoxError = Box<dyn StdError + Send + Sync>;

/// Errors produced by the weather abstraction.
///
/// The `EmptySnapshot` and `SeriesLengthMismatch`
/// variants mirror the invariants previously
/// enforced on `clients::windy::Forecast`. The
/// `Transport` and `Provider` variants are
/// escape-hatches so concrete providers can surface
/// their own errors without this enum depending on
/// every provider crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WeatherError {
    /// The snapshot had no timestamps. Every
    /// downstream consumer assumes at least one
    /// forecast step, so an empty series is an
    /// error rather than a pass-through.
    #[error("weather snapshot has no timestamps")]
    EmptySnapshot,

    /// A parallel series did not match
    /// `timestamps.len()`. Indicates wire-format
    /// drift inside a provider adapter.
    #[error(
        "series `{series}` has length {got} \
         but timestamps has length {expected}"
    )]
    SeriesLengthMismatch {
        /// Name of the field that failed the length
        /// check (e.g., `"temperature_c"`).
        series: String,
        /// `timestamps.len()`.
        expected: usize,
        /// The series' actual length.
        got: usize,
    },

    /// HTTP / TLS / decode failure from a provider's
    /// underlying transport. Message passes through
    /// the inner error verbatim — `PublishError` adds
    /// the `"fetching weather forecast:"` framing
    /// one level up, so doubling the prefix here
    /// would produce noisy logs.
    #[error("{0}")]
    Transport(BoxError),

    /// Provider-specific parse or API-level error
    /// (e.g., a non-2xx response, malformed JSON,
    /// unexpected units). The inner error is opaque
    /// so the neutral layer does not depend on any
    /// provider crate. See [`Self::Transport`] for
    /// the reasoning on pass-through messages.
    #[error("{0}")]
    Provider(BoxError),
}
