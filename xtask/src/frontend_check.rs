//! Runs `svelte-check` against the frontend TypeScript +
//! Svelte sources. Skipped gracefully when the frontend
//! is absent (the template's web crate is optional).

use std::path::Path;
use std::process::Command;

/// Outcome of a frontend check run.
pub struct FrontendCheckResult {
    /// Human-readable detail for the step line.
    pub detail: String,
    /// Error message if the check failed.
    pub error: Option<String>,
    /// True when the check was skipped (no frontend).
    pub skipped: bool,
}

/// Run `npm run check` in `frontend/`.
///
/// Returns `skipped=true` when `frontend/package.json`
/// does not exist, or when `frontend/node_modules` has
/// not been installed yet.
pub fn frontend_check() -> Result<FrontendCheckResult, String> {
    if !Path::new("frontend/package.json").exists() {
        return Ok(FrontendCheckResult {
            detail: "no frontend".into(),
            error: None,
            skipped: true,
        });
    }
    if !Path::new("frontend/node_modules").exists() {
        return Ok(FrontendCheckResult {
            detail: "node_modules missing".into(),
            error: None,
            skipped: true,
        });
    }

    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let output = Command::new(npm)
        .args(["--prefix", "frontend", "run", "check"])
        .output()
        .map_err(|e| format!("failed to run {npm}: {e}"))?;

    if output.status.success() {
        Ok(FrontendCheckResult {
            detail: String::new(),
            error: None,
            skipped: false,
        })
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}{stderr}");
        for line in combined.lines().take(10) {
            eprintln!("  {line}");
        }
        Ok(FrontendCheckResult {
            detail: String::new(),
            error: Some("svelte-check reported errors".into()),
            skipped: false,
        })
    }
}
