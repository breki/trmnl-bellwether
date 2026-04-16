use std::fmt::Write as _;

use crate::helpers::run_cargo_capture;

/// Minimum line coverage percentage (overall).
pub const THRESHOLD: f64 = 90.0;

/// Per-module coverage floor.
const MODULE_THRESHOLD: f64 = 85.0;

/// Regex passed to `llvm-cov --ignore-filename-regex`.
/// Excludes direct `src/main.rs` entry points (thin
/// bootstrap wrappers) on both Unix and Windows path
/// separators.
const IGNORE_REGEX: &str = r"src[/\\]main\.rs$";

/// Coverage check result for use by validate.
pub struct CoverageResult {
    /// Overall line coverage percentage.
    pub line_pct: f64,
    /// Covered lines.
    pub covered: u64,
    /// Total lines.
    pub total: u64,
    /// Error message if failed (None = passed).
    pub error: Option<String>,
}

/// Run coverage check and return structured result.
pub fn coverage_check() -> Result<CoverageResult, String> {
    let output = run_cargo_capture(&[
        "llvm-cov",
        "--workspace",
        "--exclude",
        "xtask",
        "--ignore-filename-regex",
        IGNORE_REGEX,
        "--json",
        "--summary-only",
    ])?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cargo llvm-cov failed:\n{stderr}"));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse coverage JSON: {e}"))?;

    let totals = &json["data"][0]["totals"]["lines"];
    let line_pct = totals["percent"].as_f64().ok_or("missing lines.percent")?;
    let covered = totals["covered"].as_u64().ok_or("missing lines.covered")?;
    let total = totals["count"].as_u64().ok_or("missing lines.count")?;

    // Check per-module coverage.
    let mut below = Vec::new();
    if let Some(files) = json["data"][0]["files"].as_array() {
        for file in files {
            let name = file["filename"].as_str().unwrap_or("?");
            let pct =
                file["summary"]["lines"]["percent"].as_f64().unwrap_or(0.0);
            let count = file["summary"]["lines"]["count"].as_u64().unwrap_or(0);
            let short = shorten_path(name);
            if count > 0 && pct < MODULE_THRESHOLD {
                below.push((short.to_string(), pct));
            }
        }
    }

    if line_pct < THRESHOLD {
        Ok(CoverageResult {
            line_pct,
            covered,
            total,
            error: Some(format!(
                "coverage {line_pct:.1}% is below \
                 {THRESHOLD}% threshold"
            )),
        })
    } else if !below.is_empty() {
        let mut msg = String::from("modules below coverage threshold:");
        for (name, pct) in &below {
            let _ = write!(msg, "\n    {name}: {pct:.1}%");
        }
        Ok(CoverageResult {
            line_pct,
            covered,
            total,
            error: Some(msg),
        })
    } else {
        Ok(CoverageResult {
            line_pct,
            covered,
            total,
            error: None,
        })
    }
}

/// Run coverage check with printed output (standalone).
pub fn coverage() -> Result<(), String> {
    let r = coverage_check()?;
    println!("  lines: {}/{} ({:.1}%)", r.covered, r.total, r.line_pct);
    if let Some(err) = r.error {
        Err(err)
    } else {
        println!("  coverage OK ({:.1}% >= {THRESHOLD}%)", r.line_pct);
        Ok(())
    }
}

/// Shorten a file path to just the part after `src/`.
fn shorten_path(name: &str) -> &str {
    name.rsplit_once("src\\")
        .or_else(|| name.rsplit_once("src/"))
        .map_or(name, |(_, rest)| rest)
}
