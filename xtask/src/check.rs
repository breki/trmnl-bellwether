use crate::helpers::run_cargo_capture;

/// Maximum number of error lines to display.
const MAX_ERROR_LINES: usize = 10;

/// Number of stderr tail lines to print when the cargo
/// invocation fails but no rustc error lines were
/// matched (e.g. manifest parse failure, corrupted
/// `Cargo.lock`, missing network).
const STDERR_TAIL_LINES: usize = 20;

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

    if count == 0 {
        // Non-rustc failure (bad manifest, corrupted lock,
        // network, ...). The error-prefix filter matches
        // nothing, so dump the stderr tail verbatim so the
        // user sees something actionable.
        eprintln!(
            "FAILED: cargo exited non-zero with no matched error lines\n"
        );
        let tail = stderr_tail(&stderr, STDERR_TAIL_LINES);
        for line in tail {
            eprintln!("  {line}");
        }
        return Err("cargo check failed (see stderr above)".to_string());
    }

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
/// the rustc summary line `error: aborting due to N
/// previous errors`. The filter is anchored to the
/// exact summary prefix so user errors whose message
/// happens to contain the substring "aborting" are not
/// dropped.
fn extract_error_lines(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|l| l.starts_with("error[") || l.starts_with("error:"))
        .filter(|l| !l.starts_with("error: aborting due to"))
        .collect()
}

/// Return the last `n` non-empty lines of `stderr`.
fn stderr_tail(stderr: &str, n: usize) -> Vec<&str> {
    let mut lines: Vec<&str> =
        stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    lines.drain(..start);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_STDERR: &str = "\
error[E0425]: cannot find value `foo` in this scope
 --> crates/bellwether/src/lib.rs:45:12
error[E0308]: mismatched types
 --> crates/bellwether-web/src/api/mod.rs:123:5
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
    fn keeps_user_errors_mentioning_aborting() {
        // User errors whose message contains the
        // substring "aborting" must not be filtered as
        // if they were the rustc summary line.
        let stderr = "\
error: aborting: feature X required
error: aborting due to 1 previous error";
        let errors = extract_error_lines(stderr);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("feature X required"));
    }

    #[test]
    fn stderr_tail_returns_last_n_non_empty() {
        let stderr = "one\n\ntwo\nthree\n\nfour\n";
        let tail = stderr_tail(stderr, 2);
        assert_eq!(tail, vec!["three", "four"]);
    }

    #[test]
    fn stderr_tail_handles_short_input() {
        let stderr = "only-one";
        let tail = stderr_tail(stderr, 5);
        assert_eq!(tail, vec!["only-one"]);
    }

    #[test]
    fn includes_plain_error_lines() {
        let stderr = "\
error[E0425]: cannot find value `foo`
error: could not compile `bellwether`
error: aborting due to 1 previous error";
        let errors = extract_error_lines(stderr);
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("E0425"));
        assert!(errors[1].contains("could not compile"));
    }
}
