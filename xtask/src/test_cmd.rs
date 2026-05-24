use crate::helpers::{run_cargo_capture, run_cargo_stream};

/// Maximum failure detail lines per test.
const MAX_DETAIL_LINES: usize = 5;

/// Test invocation scope.
#[derive(Clone, Copy)]
enum Scope {
    /// `--workspace` -- every crate in the workspace.
    Workspace,
    /// `-p xtask` -- only the xtask crate. Used by
    /// validate's Test step (see module docs there).
    XtaskOnly,
}

/// Knobs for [`test`]. Struct rather than a positional
/// bool pair so the call site stays readable when more
/// harness flags land (e.g. `include_ignored`,
/// `test_threads`, …) and a future caller can't
/// silently swap `verbose` and `ignored`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TestOptions<'a> {
    /// Optional substring filter passed to cargo test.
    pub filter: Option<&'a str>,
    /// Stream raw cargo test output instead of the
    /// compact summary.
    pub verbose: bool,
    /// Run only tests marked `#[ignore]`.
    pub ignored: bool,
}

/// Run tests with concise output.
///
/// Prints `Test OK` on success. On failure, shows only
/// the failing test names and assertion details.
/// With `opts.verbose`, streams raw cargo test output.
/// With `opts.ignored`, runs only `#[ignore]`-marked
/// tests.
pub fn test(opts: TestOptions<'_>) -> Result<(), String> {
    let args = build_args(Scope::Workspace, opts.filter, opts.ignored)?;

    if opts.verbose {
        return run_cargo_stream(&args);
    }

    let output = run_cargo_capture(&args)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        println!("Test OK");
        return Ok(());
    }

    report_failure(&stdout, &stderr)
}

/// Run only xtask's own tests quietly.
///
/// Used by validate's Test step. Coverage runs the
/// workspace test suite under llvm-cov instrumentation
/// (with `--exclude xtask`), so running the full
/// workspace tests separately in Test would duplicate
/// the same passes. Restricting Test to `-p xtask`
/// keeps xtask covered without re-running every other
/// crate.
///
/// On failure, prints the same rich diagnostics as
/// `test()` to stderr (failing names, assertion
/// details, or compile errors) before returning.
pub fn test_check_xtask() -> Result<(), String> {
    let args = build_args(Scope::XtaskOnly, None, false)?;
    let output = run_cargo_capture(&args)?;

    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    report_failure(&stdout, &stderr)
}

/// Print failure diagnostics to stderr and return the
/// corresponding error string. Shared between `test()`
/// and `test_check_xtask()`.
fn report_failure(stdout: &str, stderr: &str) -> Result<(), String> {
    // Compilation error -- show first error lines.
    if stderr.contains("could not compile") {
        let errors: Vec<&str> = stderr
            .lines()
            .filter(|l| l.starts_with("error"))
            .take(10)
            .collect();
        eprintln!("FAILED: compilation error\n");
        for line in &errors {
            eprintln!("  {line}");
        }
        return Err("compilation failed".into());
    }

    // Test failures -- show failing names + details.
    let failed_names = extract_failed_names(stdout);
    let failures = extract_failure_details(stdout, stderr);

    eprintln!("FAILED\n");
    if failures.is_empty() {
        for name in &failed_names {
            eprintln!("  {name}");
        }
    } else {
        for f in &failures {
            eprintln!("  {}", f.name);
            for d in f.details.iter().take(MAX_DETAIL_LINES) {
                eprintln!("    {d}");
            }
        }
    }
    Err("test(s) failed".into())
}

/// Build the cargo test argument list.
///
/// Centralised so both the CLI `test` command and
/// validate's xtask-only test step go through the
/// same arg-construction path. `ignored` maps to
/// cargo test's `--ignored` harness flag (runs only
/// `#[ignore]`-marked tests); the flag must appear
/// after the `--` separator so cargo forwards it to
/// the test binary rather than trying to parse it
/// itself.
fn build_args(
    scope: Scope,
    filter: Option<&str>,
    ignored: bool,
) -> Result<Vec<&str>, String> {
    if let Some(f) = filter
        && f.is_empty()
    {
        return Err("test filter must not be empty".into());
    }

    let mut args = vec!["test"];
    match scope {
        Scope::Workspace => args.push("--workspace"),
        Scope::XtaskOnly => {
            args.push("-p");
            args.push("xtask");
        }
    }
    if ignored || filter.is_some() {
        args.push("--");
    }
    if let Some(f) = filter {
        args.push(f);
    }
    if ignored {
        args.push("--ignored");
    }
    Ok(args)
}

/// Extract test names from `test foo ... FAILED` lines.
fn extract_failed_names(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| l.trim().ends_with("... FAILED"))
        .map(|l| {
            l.trim()
                .strip_prefix("test ")
                .unwrap_or(l.trim())
                .strip_suffix(" ... FAILED")
                .unwrap_or(l.trim())
                .to_string()
        })
        .collect()
}

/// A single test failure with detail lines.
struct FailureDetail {
    /// Fully qualified test name.
    name: String,
    /// Assertion detail lines (panic message, etc.).
    details: Vec<String>,
}

/// Extract failing test details from
/// `---- name stdout ----` sections.
fn extract_failure_details(stdout: &str, stderr: &str) -> Vec<FailureDetail> {
    let mut failures = Vec::new();
    let combined = format!("{stdout}\n{stderr}");

    let mut current_name: Option<String> = None;
    let mut current_details: Vec<String> = Vec::new();

    for line in combined.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("---- ") {
            if let Some(name) = current_name.take() {
                failures.push(FailureDetail {
                    name,
                    details: std::mem::take(&mut current_details),
                });
            }
            if let Some(name) = rest.strip_suffix(" stdout ----") {
                current_name = Some(name.to_string());
            }
        } else if current_name.is_some()
            && !trimmed.is_empty()
            && !trimmed.starts_with("thread '")
            && !trimmed.starts_with("note: run with")
        {
            current_details.push(trimmed.to_string());
        }
    }

    if let Some(name) = current_name.take() {
        failures.push(FailureDetail {
            name,
            details: current_details,
        });
    }

    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_workspace_no_filter() {
        let args = build_args(Scope::Workspace, None, false).unwrap();
        assert_eq!(args, vec!["test", "--workspace"]);
    }

    #[test]
    fn build_args_xtask_only() {
        let args = build_args(Scope::XtaskOnly, None, false).unwrap();
        assert_eq!(args, vec!["test", "-p", "xtask"]);
    }

    #[test]
    fn build_args_filter_puts_separator_before_name() {
        let args = build_args(Scope::Workspace, Some("foo"), false).unwrap();
        assert_eq!(args, vec!["test", "--workspace", "--", "foo"]);
    }

    #[test]
    fn build_args_ignored_alone_still_gets_separator() {
        // `--ignored` is a harness flag; it must land
        // after `--` so cargo forwards it to the test
        // binary rather than trying to parse it itself.
        let args = build_args(Scope::Workspace, None, true).unwrap();
        assert_eq!(args, vec!["test", "--workspace", "--", "--ignored"]);
    }

    #[test]
    fn build_args_filter_and_ignored_compose() {
        let args = build_args(Scope::Workspace, Some("foo"), true).unwrap();
        assert_eq!(args, vec!["test", "--workspace", "--", "foo", "--ignored"]);
    }

    #[test]
    fn build_args_empty_filter_errors() {
        assert!(build_args(Scope::Workspace, Some(""), false).is_err());
    }

    #[test]
    fn build_args_empty_filter_errors_even_with_ignored() {
        // An empty filter must short-circuit before any
        // output is produced, regardless of what other
        // harness flags the caller asked for. Pins the
        // validate-first invariant so a future refactor
        // can't silently emit a half-built command.
        assert!(build_args(Scope::Workspace, Some(""), true).is_err());
    }

    #[test]
    fn extract_failed_names_from_output() {
        let stdout = "\
test foo::bar ... ok
test baz::qux ... FAILED
test another::test ... ok
test third::fail ... FAILED";
        let names = extract_failed_names(stdout);
        assert_eq!(names.len(), 2);
        assert_eq!(names[0], "baz::qux");
        assert_eq!(names[1], "third::fail");
    }

    #[test]
    fn extract_failed_names_none() {
        let stdout = "test foo::bar ... ok\n\
            test result: ok. 1 passed";
        let names = extract_failed_names(stdout);
        assert!(names.is_empty());
    }

    #[test]
    fn extract_details_from_output() {
        let stdout = "\
test api::tests::my_test ... FAILED

failures:

---- api::tests::my_test stdout ----
thread 'api::tests::my_test' panicked at 'msg'
assertion `left == right` failed
  left: 1
 right: 2
note: run with RUST_BACKTRACE=1

failures:
    api::tests::my_test
";
        let failures = extract_failure_details(stdout, "");
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].name, "api::tests::my_test");
        assert!(
            failures[0].details.iter().any(|d| d.contains("assertion")),
            "should contain assertion detail"
        );
        assert!(
            !failures[0]
                .details
                .iter()
                .any(|d| d.starts_with("thread '")),
            "should not contain thread line"
        );
    }
}
