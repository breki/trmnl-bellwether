use crate::helpers::run_cargo_capture;

/// Maximum number of error lines to display.
const MAX_ERROR_LINES: usize = 10;

/// Run `cargo check` with concise output.
///
/// Prints `Check OK` on success or `FAILED: N error(s)`
/// with the first few error lines on failure.
pub fn check() -> Result<(), String> {
    let output =
        run_cargo_capture(&["check", "--workspace", "--message-format=short"])?;

    if output.status.success() {
        println!("Check OK");
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let errors = extract_error_lines(&stderr);
    let count = errors.len();

    eprintln!("FAILED: {count} compilation error(s)\n");
    for line in errors.iter().take(MAX_ERROR_LINES) {
        eprintln!("  {line}");
    }
    if count > MAX_ERROR_LINES {
        eprintln!("  ... and {} more", count - MAX_ERROR_LINES);
    }
    Err(format!("{count} compilation error(s)"))
}

/// Extract error lines from cargo check stderr.
///
/// Matches `error[E...]` and `error:` lines, excluding
/// the `aborting` summary line.
fn extract_error_lines(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|l| l.starts_with("error[") || l.starts_with("error:"))
        .filter(|l| !l.contains("aborting"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_STDERR: &str = "\
error[E0425]: cannot find value `foo` in this scope
 --> crates/rustbase/src/lib.rs:45:12
error[E0308]: mismatched types
 --> crates/rustbase-web/src/api/mod.rs:123:5
warning: unused variable: `x`
 --> xtask/src/main.rs:10:9
error: aborting due to 2 previous errors";

    #[test]
    fn extracts_only_error_bracket_lines() {
        let errors = extract_error_lines(SAMPLE_STDERR);
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("E0425"));
        assert!(errors[1].contains("E0308"));
    }

    #[test]
    fn empty_input_gives_empty_result() {
        let errors = extract_error_lines("");
        assert!(errors.is_empty());
    }

    #[test]
    fn warnings_only_gives_empty_result() {
        let stderr = "warning: unused variable: `x`";
        let errors = extract_error_lines(stderr);
        assert!(errors.is_empty());
    }

    #[test]
    fn includes_plain_error_lines() {
        let stderr = "\
error[E0425]: cannot find value `foo`
error: could not compile `rustbase`
error: aborting due to 1 previous error";
        let errors = extract_error_lines(stderr);
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("E0425"));
        assert!(errors[1].contains("could not compile"));
    }
}
