//! Shared HTTP helpers for the provider clients.
//!
//! The provider clients both need to: build a
//! `reqwest::Client` with consistent timeouts, read a
//! response body with a byte cap, and truncate
//! oversize error bodies for diagnostics. Extracted
//! here so those helpers live in one place and the
//! per-provider modules stay focused on wire-format
//! concerns.

use std::string::FromUtf8Error;
use std::time::Duration;

use reqwest::redirect::Policy;

/// Build the canonical `reqwest::Client` used by every
/// provider: 5 s connect / 20 s total timeouts,
/// bounded redirect follow, `bellwether/<version>` user
/// agent.
///
/// The redirect limit is deliberately small (3 hops)
/// and applies only to same-scheme HTTPS → HTTPS
/// redirects that reqwest follows by default — enough
/// for a CDN canonical-host bounce but not a
/// run-away redirect chain. We do NOT use
/// `Policy::none()` because a legitimate 3xx from
/// Open-Meteo's CDN would otherwise surface as an
/// opaque `Api { status: 301, body: "" }` error.
///
/// Panics only if the OS's TLS init fails — a
/// process-wide condition with no meaningful
/// recovery.
#[must_use]
pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("bellwether/", env!("CARGO_PKG_VERSION"),))
        .redirect(Policy::limited(3))
        .build()
        .expect("reqwest builder with rustls-tls is infallible")
}

/// Errors [`read_capped_body`] can produce. Providers
/// convert each variant into their own error type via
/// `From`.
#[derive(Debug, thiserror::Error)]
pub enum ReadBodyError {
    /// Transport-level failure reading a chunk.
    #[error("{0}")]
    Transport(#[from] reqwest::Error),
    /// The body (or its advertised `Content-Length`)
    /// was larger than the cap.
    #[error("response body exceeded {limit} bytes")]
    TooLarge {
        /// The cap that was exceeded.
        limit: u64,
    },
    /// The body bytes were not valid UTF-8. Preserves
    /// the real [`FromUtf8Error`] so the byte offset
    /// and invalid-byte diagnostic survive the error
    /// chain.
    #[error("response body was not valid UTF-8: {0}")]
    NotUtf8(#[from] FromUtf8Error),
}

/// Read a response body into a `String`, rejecting
/// anything larger than `limit` bytes.
///
/// The cap is enforced *before* each chunk is copied
/// into the accumulator so a single oversize HTTP/2
/// frame cannot force an allocation past `limit`.
/// Short-circuits on `Content-Length` when the server
/// advertises it.
pub async fn read_capped_body(
    mut resp: reqwest::Response,
    limit: u64,
) -> Result<String, ReadBodyError> {
    if let Some(len) = resp.content_length()
        && len > limit
    {
        return Err(ReadBodyError::TooLarge { limit });
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut total: u64 = 0;
    while let Some(chunk) = resp.chunk().await? {
        let chunk_len = chunk.len() as u64;
        if total.saturating_add(chunk_len) > limit {
            return Err(ReadBodyError::TooLarge { limit });
        }
        total = total.saturating_add(chunk_len);
        buf.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8(buf)?)
}

/// Truncate a string at a UTF-8 boundary, appending
/// `"…(truncated)"` if anything was dropped.
pub fn truncate_with_ellipsis(mut s: String, max_len: usize) -> String {
    if s.len() <= max_len {
        return s;
    }
    let mut cut = max_len;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
    s.push_str("…(truncated)");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_string_is_unchanged() {
        let s = "hello".to_owned();
        assert_eq!(truncate_with_ellipsis(s.clone(), 512), s);
    }

    #[test]
    fn truncate_long_string_appends_marker() {
        let s = "a".repeat(600);
        let out = truncate_with_ellipsis(s, 512);
        assert!(out.ends_with("…(truncated)"));
        assert!(out.len() <= 512 + "…(truncated)".len());
    }

    #[test]
    fn truncate_respects_utf8_char_boundaries() {
        let s = "aéaéaé".to_owned();
        let out = truncate_with_ellipsis(s, 3);
        assert!(out.ends_with("…(truncated)"));
    }

    #[test]
    fn build_http_client_returns_usable_client() {
        let _ = build_http_client();
    }

    #[test]
    fn read_body_error_display_and_source() {
        use std::error::Error;

        let too_large = ReadBodyError::TooLarge { limit: 1024 };
        assert!(too_large.to_string().contains("1024"));

        // Construct a FromUtf8Error to exercise the
        // NotUtf8 variant's Display and source chain.
        let bad = String::from_utf8(vec![0xFFu8]).unwrap_err();
        let wrapped = ReadBodyError::NotUtf8(bad);
        assert!(wrapped.to_string().contains("not valid UTF-8"));
        assert!(wrapped.source().is_some());
    }
}
