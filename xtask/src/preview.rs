//! `cargo xtask preview` — regenerate the dashboard
//! sample artefacts and serve them over HTTP for
//! browser-based eyeball inspection.
//!
//! Three panels are produced by the underlying
//! `generate_dashboard_sample` test
//! (`crates/bellwether/src/publish/tests.rs`):
//!
//! - **SVG** — the renderer's input.
//! - **PNG** — `resvg` raster, pre-dither.
//! - **BMP** — final 1-bit output sent to the TRMNL.
//!
//! Seeing all three side-by-side makes it possible to
//! isolate a visual regression to either the SVG
//! layout, the rasterizer, or the Floyd–Steinberg
//! dither in one glance instead of diffing BMPs.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

/// Filenames of the three artefacts produced fresh by
/// each `cargo xtask preview` run (excluding the
/// static HTML viewer, which the xtask writes
/// directly). Used to verify that the ignored test
/// actually ran before the server starts serving —
/// if the test name drifts, `cargo test` silently
/// matches zero tests and the server would otherwise
/// happily serve yesterday's stale dashboard.
const SAMPLE_ARTEFACTS: &[&str] = &[
    "dashboard-sample.svg",
    "dashboard-sample.png",
    "dashboard-sample.bmp",
];

/// Regenerate artefacts, write the viewer HTML, bind
/// the listener, optionally launch the browser, and
/// then enter the request-handling loop until Ctrl-C.
///
/// The listener is bound **before** `--open` fires so
/// the browser never races the server's readiness.
pub fn preview(port: u16, open: bool) -> Result<(), String> {
    let target = workspace_target_dir()?;
    regenerate_sample(&target)?;

    let index_path = target.join("preview-index.html");
    std::fs::write(&index_path, INDEX_HTML)
        .map_err(|e| format!("writing {}: {}", index_path.display(), e))?;

    // Bind before announcing / opening — otherwise a
    // fast browser issues its GET before the socket
    // is listening and gets ECONNREFUSED.
    let server = tiny_http::Server::http(("127.0.0.1", port))
        .map_err(|e| format!("binding 127.0.0.1:{port}: {e}"))?;

    // Print `127.0.0.1` rather than `localhost` to
    // match the bind address — `localhost` can resolve
    // to `::1` first on some Windows setups and time
    // out before falling back to IPv4.
    let url = format!("http://127.0.0.1:{port}/preview-index.html");
    println!("→ {url}");
    println!("  serving {}", target.display());
    println!("  (Ctrl-C to stop)");
    if open && let Err(e) = open_browser(&url) {
        eprintln!("  warning: could not open browser: {e}");
    }

    for request in server.incoming_requests() {
        handle_request(&target, request);
    }
    Ok(())
}

/// Invoke the `generate_dashboard_sample` ignored test
/// that produces `target/dashboard-sample.{svg,png,bmp}`.
/// Shells out to `cargo test` because the test lives in
/// `#[cfg(test)]` code that xtask shouldn't link
/// against directly.
///
/// Passes `--exact` with the fully-qualified test path
/// so substring matches against future unrelated tests
/// don't sneak in. After the test returns, stats each
/// artefact's mtime — if any predates the start of this
/// function, the test didn't actually write it (e.g.
/// it was renamed, `cargo test` matched zero tests and
/// still exited 0) and we refuse to serve stale output.
fn regenerate_sample(target: &Path) -> Result<(), String> {
    println!("→ regenerating dashboard sample (cargo test --ignored)");
    let start = SystemTime::now();
    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "bellwether",
            "--lib",
            "publish::tests::generate_dashboard_sample",
            "--",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .status()
        .map_err(|e| format!("invoking cargo test: {e}"))?;
    if !status.success() {
        return Err(format!("sample generation failed (exit {status})"));
    }
    for name in SAMPLE_ARTEFACTS {
        let path = target.join(name);
        let modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .map_err(|e| format!("stat {}: {}", path.display(), e))?;
        if modified < start {
            return Err(format!(
                "{} was not regenerated — test name likely drifted, \
                 `cargo test` matched zero tests and exited 0",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Subset of `cargo metadata --format-version 1` the
/// xtask needs. A typed struct instead of
/// `serde_json::Value` indexing catches schema drift
/// at the deserialisation boundary rather than two
/// layers deeper in a stringly-typed chain.
#[derive(serde::Deserialize)]
struct CargoMetadata {
    target_directory: PathBuf,
}

/// Resolve the workspace's `target/` directory via
/// `cargo metadata` — robust to being invoked from any
/// subdirectory and to `CARGO_TARGET_DIR` overrides.
fn workspace_target_dir() -> Result<PathBuf, String> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let meta: CargoMetadata = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("parsing cargo metadata: {e}"))?;
    Ok(meta.target_directory)
}

/// Handle a single preview request.
///
/// Uses a hardcoded filename allowlist (not a
/// canonicalised directory prefix check) so there's no
/// path-traversal surface to analyse — unknown URLs
/// short-circuit to 404 before any filesystem access.
fn handle_request(root: &Path, request: tiny_http::Request) {
    let relative = request.url().trim_start_matches('/');
    let name = if relative.is_empty() {
        "preview-index.html"
    } else {
        relative
    };
    let Some(mime) = allowed_mime(name) else {
        let _ = request.respond(tiny_http::Response::empty(404));
        return;
    };
    let fs_path = root.join(name);
    if let Ok(bytes) = std::fs::read(&fs_path) {
        let header = tiny_http::Header::from_bytes(
            &b"Content-Type"[..],
            mime.as_bytes(),
        )
        .expect("static mime string always parses");
        let response =
            tiny_http::Response::from_data(bytes).with_header(header);
        let _ = request.respond(response);
    } else {
        let _ = request.respond(tiny_http::Response::empty(404));
    }
}

/// Allowlist-plus-MIME lookup. Returning `None` means
/// "not allowed" — there are no wildcards and no file
/// outside this match is reachable via the preview
/// server. This is the entire path-traversal defence,
/// so keep the arms explicit and narrow.
fn allowed_mime(name: &str) -> Option<&'static str> {
    match name {
        "preview-index.html" => Some("text/html; charset=utf-8"),
        "dashboard-sample.svg" => Some("image/svg+xml"),
        "dashboard-sample.png" => Some("image/png"),
        "dashboard-sample.bmp" => Some("image/bmp"),
        _ => None,
    }
}

/// Cross-platform browser launcher. Per-OS `cfg` arms
/// keep the binary size small (no `open`/`opener`
/// crate) and match each OS's native "open URL"
/// convention.
fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        // SAFETY-BY-CONTRACT: `url` must not contain
        // cmd.exe metacharacters (`&`, `|`, `^`, `<`,
        // `>`, `"`). Current callers format `url` from
        // a numeric port plus a hardcoded path, so the
        // invariant holds. If a future caller threads
        // user-controlled text into the URL, switch to
        // `ShellExecuteW` via the `windows` crate or
        // `rundll32 url.dll,FileProtocolHandler <url>`
        // — `cmd /C start` inherits cmd.exe's
        // metacharacter re-parse, which Rust's
        // `CreateProcessW`-targeted escaping does not
        // cover.
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()
            .map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).status().map(|_| ())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(url).status().map(|_| ())
    }
}

/// HTML viewer served at `/preview-index.html`. Kept
/// inline because it's small and this way `xtask
/// preview` has no runtime asset-path dependency; a
/// `cargo install --path xtask` would work anywhere.
const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>bellwether preview</title>
<style>
  body { margin: 0; background: #1a1a1a; color: #ddd;
         font: 14px/1.4 system-ui, sans-serif; }
  header { padding: 1rem 1.5rem; border-bottom: 1px solid #333; }
  h1 { margin: 0; font-size: 1rem; font-weight: 500; }
  main { padding: 2rem; display: flex; flex-direction: column;
         gap: 2rem; align-items: center; }
  figure { margin: 0; text-align: center; }
  figcaption { font-size: 0.85rem; opacity: 0.7;
               margin-top: 0.5rem; max-width: 60ch; }
  img { background: white; box-shadow: 0 4px 12px rgba(0,0,0,0.5);
        max-width: 100%; height: auto; display: block; }
</style>
</head>
<body>
<header><h1>bellwether preview — sample dashboard</h1></header>
<main>
<figure>
  <img src="dashboard-sample.svg" width="800" height="480"
       alt="raw SVG">
  <figcaption>Raw SVG — the renderer's input. No rasterisation.</figcaption>
</figure>
<figure>
  <img src="dashboard-sample.png" width="800" height="480"
       alt="resvg raster, pre-dither">
  <figcaption>resvg raster, pre-dither. Any regression visible here
    but not in the SVG points at the rasteriser.</figcaption>
</figure>
<figure>
  <img src="dashboard-sample.bmp" width="800" height="480"
       alt="final 1-bit BMP">
  <figcaption>Final 1-bit BMP — what the TRMNL actually sees.
    Differences from the PNG above isolate the Floyd–Steinberg
    dither contribution.</figcaption>
</figure>
</main>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_mime_covers_every_artefact_and_advertises_correct_type() {
        // SVG must be `image/svg+xml` so browsers render
        // it inline; BMP must not fall through to
        // octet-stream, which would offer it as a
        // download instead of showing it.
        assert_eq!(
            allowed_mime("preview-index.html"),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(allowed_mime("dashboard-sample.svg"), Some("image/svg+xml"));
        assert_eq!(allowed_mime("dashboard-sample.png"), Some("image/png"));
        assert_eq!(allowed_mime("dashboard-sample.bmp"), Some("image/bmp"));
    }

    #[test]
    fn allowed_mime_rejects_everything_off_the_allowlist() {
        // The allowlist is the entire path-traversal
        // defence. Files one would naively assume
        // should be reachable under a static server —
        // debug binaries, build outputs — must not be.
        for forbidden in &[
            "debug/bellwether",
            "../../etc/passwd",
            "Cargo.toml",
            "dashboard-sample.exe",
            "dashboard-sample",
            "",
        ] {
            assert_eq!(
                allowed_mime(forbidden),
                None,
                "unexpectedly allowed: {forbidden}"
            );
        }
    }

    #[test]
    fn sample_artefacts_all_have_allowlist_entries() {
        // The regenerate + mtime-verify loop asserts
        // the sample artefacts were written; if any
        // weren't also in `allowed_mime` the preview
        // server would 404 them. Locks the two
        // declarations together so future edits can't
        // diverge silently.
        for name in SAMPLE_ARTEFACTS {
            assert!(
                allowed_mime(name).is_some(),
                "SAMPLE_ARTEFACTS entry {name:?} lacks a MIME mapping"
            );
        }
    }

    #[test]
    fn index_html_references_all_three_preview_artefacts() {
        // The viewer exists to show all three; if the
        // filenames ever drift between the generator
        // test, SAMPLE_ARTEFACTS, and the HTML, the
        // preview would silently show stale or missing
        // images.
        for name in SAMPLE_ARTEFACTS {
            assert!(
                INDEX_HTML.contains(name),
                "INDEX_HTML does not reference {name:?}"
            );
        }
    }
}
