use std::time::Instant;

use crate::clippy_cmd;
use crate::coverage;
use crate::dupes;
use crate::fmt_cmd;
use crate::frontend_check;
use crate::helpers::{elapsed_str, step_output};
use crate::test_cmd;

/// Total number of validation steps.
const TOTAL_STEPS: usize = 6;

/// Run all validation steps with concise stepwise
/// output.
pub fn validate() -> Result<(), String> {
    let overall_start = Instant::now();

    // 1. Fmt
    run_step(1, "Fmt", run_fmt)?;

    // 2. Clippy
    run_step(2, "Clippy", run_clippy)?;

    // 3. Test
    run_step(3, "Test", run_test)?;

    // 4. Coverage
    run_step(4, "Coverage", run_coverage)?;

    // 5. Duplication
    run_step(5, "Duplication", run_duplication)?;

    // 6. Frontend type check (skipped if no frontend)
    run_step(6, "Frontend", run_frontend_check)?;

    println!("Validate OK ({})", elapsed_str(overall_start));
    Ok(())
}

/// Run a single step, printing the `[N/T]` result line.
fn run_step(
    step: usize,
    name: &str,
    f: fn() -> Result<String, String>,
) -> Result<(), String> {
    let start = Instant::now();
    match f() {
        Ok(detail) => {
            let time = elapsed_str(start);
            let full = if detail.is_empty() {
                time
            } else {
                format!("{detail}, {time}")
            };
            step_output(step, TOTAL_STEPS, name, "OK", &full);
            Ok(())
        }
        Err(e) => {
            step_output(step, TOTAL_STEPS, name, "FAILED", "");
            Err(e)
        }
    }
}

/// Fmt step -- returns empty detail on success.
fn run_fmt() -> Result<String, String> {
    fmt_cmd::fmt_check()?;
    Ok(String::new())
}

/// Clippy step -- returns empty detail on success.
fn run_clippy() -> Result<String, String> {
    let r = clippy_cmd::clippy_check()?;
    match r.error {
        None => Ok(String::new()),
        Some(err) => {
            for line in r.items.iter().take(5) {
                eprintln!("  {line}");
            }
            Err(err)
        }
    }
}

/// Test step -- returns empty detail on success.
fn run_test() -> Result<String, String> {
    test_cmd::test_check(None)?;
    Ok(String::new())
}

/// Coverage step -- returns "N.N% >= 90%" detail.
fn run_coverage() -> Result<String, String> {
    let r = coverage::coverage_check()?;
    match r.error {
        None => Ok(format!("{:.1}% >= {}%", r.line_pct, coverage::THRESHOLD,)),
        Some(err) => Err(err),
    }
}

/// Duplication step -- returns detail string.
fn run_duplication() -> Result<String, String> {
    let r = dupes::dupes_check()?;
    if let Some(err) = r.error {
        Err(err)
    } else {
        Ok(r.detail)
    }
}

/// Frontend type check -- skips gracefully when there is
/// no frontend or `node_modules` to check against.
fn run_frontend_check() -> Result<String, String> {
    let r = frontend_check::frontend_check()?;
    match r.error {
        None if r.skipped => Ok(format!("skipped: {}", r.detail)),
        None => Ok(r.detail),
        Some(err) => Err(err),
    }
}
