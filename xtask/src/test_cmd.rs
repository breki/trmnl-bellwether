use crate::helpers::{run_cargo_capture, run_cargo_stream};

/// Maximum failure detail lines per test.
const MAX_DETAIL_LINES: usize = 5;

/// Run tests with concise output.
///
/// Prints `Test OK` on success. On failure, shows only
/// the failing test names and assertion details.
/// With `verbose`, streams raw cargo test output.
pub fn test(filter: Option<&str>, verbose: bool) -> Result<(), String> {
    let args = build_args(filter)?;

    if verbose {
        return run_cargo_stream(&args);
    }

    let output = run_cargo_capture(&args)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        println!("Test OK");
        return Ok(());
    }

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
    let failed_names = extract_failed_names(&stdout);
    let failures = extract_failure_details(&stdout, &stderr);

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

/// Run tests quietly, returning Ok/Err based on exit
/// code. Used by the validate module.
pub fn test_check(filter: Option<&str>) -> Result<(), String> {
    let args = build_args(filter)?;
    let output = run_cargo_capture(&args)?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("could not compile") {
            Err("compilation failed".into())
        } else {
            Err("test(s) failed".into())
        }
    }
}

/// Build the cargo test argument list.
fn build_args(filter: Option<&str>) -> Result<Vec<&str>, String> {
    let mut args = vec!["test", "--workspace"];
    if let Some(f) = filter {
        if f.is_empty() {
            return Err("test filter must not be empty".into());
        }
        args.push("--");
        args.push(f);
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
