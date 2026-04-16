use std::process::{Command, Output};
use std::time::Instant;

/// Width of the step name column (including dots).
const STEP_NAME_WIDTH: usize = 14;

/// Format a validation step result line as a string.
///
/// Returns: `[1/5] Fmt........... OK (0.3s)`
pub fn format_step(
    step: usize,
    total: usize,
    name: &str,
    status: &str,
    detail: &str,
) -> String {
    let dots = ".".repeat(STEP_NAME_WIDTH.saturating_sub(name.len()));
    if detail.is_empty() {
        format!("[{step}/{total}] {name}{dots} {status}")
    } else {
        format!(
            "[{step}/{total}] {name}{dots} {status} \
             ({detail})"
        )
    }
}

/// Print a validation step result line to stdout.
///
/// Produces: `[1/5] Fmt........... OK (0.3s)`
pub fn step_output(
    step: usize,
    total: usize,
    name: &str,
    status: &str,
    detail: &str,
) {
    println!("{}", format_step(step, total, name, status, detail));
}

/// Format elapsed time as a human-readable string.
pub fn elapsed_str(start: Instant) -> String {
    let secs = start.elapsed().as_secs_f64();
    format!("{secs:.1}s")
}

/// Resolve the cargo binary path. Prefers the `CARGO`
/// env var (set by cargo when running xtask) over a
/// PATH lookup.
pub fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

/// Run a cargo command and capture its output.
///
/// Strips ANSI codes via `CARGO_TERM_COLOR=never`.
pub fn run_cargo_capture(args: &[&str]) -> Result<Output, String> {
    let bin = cargo_bin();
    Command::new(&bin)
        .args(args)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .map_err(|e| format!("failed to run {bin}: {e}"))
}

/// Run a cargo command, streaming output to the
/// terminal. Used for `--verbose` mode and fmt.
pub fn run_cargo_stream(args: &[&str]) -> Result<(), String> {
    let bin = cargo_bin();
    let status = Command::new(&bin)
        .args(args)
        .status()
        .map_err(|e| format!("failed to run {bin}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        match status.code() {
            Some(code) => Err(format!("{bin} exited with {code}")),
            None => Err(format!("{bin} terminated by signal")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_step_with_detail() {
        assert_eq!(
            format_step(1, 5, "Fmt", "OK", "0.3s"),
            "[1/5] Fmt........... OK (0.3s)"
        );
    }

    #[test]
    fn format_step_without_detail() {
        assert_eq!(
            format_step(2, 5, "Clippy", "FAILED", ""),
            "[2/5] Clippy........ FAILED"
        );
    }

    #[test]
    fn format_step_long_name_no_overflow() {
        let result = format_step(1, 1, "VeryLongStepName", "OK", "");
        assert_eq!(result, "[1/1] VeryLongStepName OK");
    }

    #[test]
    fn elapsed_str_format() {
        let start = Instant::now();
        let result = elapsed_str(start);
        assert!(result.ends_with('s'), "should end with 's': {result}");
        assert!(result.contains('.'), "should have decimal: {result}");
    }
}
