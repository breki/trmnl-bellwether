use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
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

/// Format a byte count as a human-readable string with
/// a binary unit suffix (`B`, `KiB`, `MiB`, `GiB`,
/// `TiB`). One decimal place above `B`.
pub fn fmt_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    // Cache sizes routinely sum into the tens-of-GB
    // range, well under f64's 2^52 mantissa headroom,
    // so precision loss is not a real concern here.
    #[allow(clippy::cast_precision_loss)]
    let mut v = bytes as f64;
    let mut idx = 0;
    while v >= 1024.0 && idx + 1 < UNITS.len() {
        v /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{v:.1} {}", UNITS[idx])
    }
}

/// A non-fatal problem encountered while walking a
/// directory tree for `dir_size`. The struct carries
/// the failing path separately from the message so
/// callers can filter or re-present without re-parsing
/// strings. `Display` produces `<path>: <message>`
/// which is the form users see in tool output.
#[derive(Debug, Clone)]
pub struct DirSizeWarning {
    pub path: PathBuf,
    pub message: String,
}

impl fmt::Display for DirSizeWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl DirSizeWarning {
    fn new(path: &Path, message: impl Into<String>) -> Self {
        Self {
            path: path.to_path_buf(),
            message: message.into(),
        }
    }
}

/// Return `true` if `meta` describes a symlink (any
/// platform) or a Windows reparse point of any kind
/// (symlink or directory junction).
///
/// On non-Windows, `meta.file_type().is_symlink()` is
/// the authoritative answer. On Windows that flag is
/// only set for `IO_REPARSE_TAG_SYMLINK`, so directory
/// junctions (`IO_REPARSE_TAG_MOUNT_POINT`, created by
/// `mklink /J`) need the `FILE_ATTRIBUTE_REPARSE_POINT`
/// bit checked explicitly. Without this guard a
/// junction below `target/` could redirect a tree
/// walk or `remove_dir_all` outside the workspace.
pub fn is_reparse_or_symlink_meta(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Recursive byte sum for a path. Symlinks and Windows
/// reparse points (directory junctions) are not
/// followed and contribute zero bytes -- defends
/// against symlink-loop stack blow-up, against
/// attributing target sizes to the source, and against
/// walking arbitrary external trees behind a
/// `mklink /J`.
///
/// Returns `(total_bytes, warnings)`. Every failure
/// (including a failed `symlink_metadata` or
/// `read_dir` on `path` itself) is folded into the
/// warnings vector with its specific failing path
/// attached; this is the only error channel, so
/// callers handle warnings uniformly regardless of
/// recursion depth. Bytes from successfully walked
/// entries are still summed.
pub fn dir_size(path: &Path) -> (u64, Vec<DirSizeWarning>) {
    let mut warnings: Vec<DirSizeWarning> = Vec::new();

    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => {
            warnings.push(DirSizeWarning::new(
                path,
                format!("symlink_metadata: {e}"),
            ));
            return (0, warnings);
        }
    };

    if is_reparse_or_symlink_meta(&meta) {
        return (0, warnings);
    }
    if meta.is_file() {
        return (meta.len(), warnings);
    }

    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            warnings.push(DirSizeWarning::new(path, format!("read_dir: {e}")));
            return (0, warnings);
        }
    };

    let mut total = 0u64;
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warnings.push(DirSizeWarning::new(
                    path,
                    format!("read_dir entry: {e}"),
                ));
                continue;
            }
        };
        let (n, mut child_warnings) = dir_size(&entry.path());
        total += n;
        warnings.append(&mut child_warnings);
    }
    (total, warnings)
}

/// Per-test scratch directory under the system temp.
/// PID + a process-wide atomic counter keep parallel
/// test runs from colliding without adding a
/// `tempfile` dependency. The counter is shared across
/// threads, so no per-thread id is needed. Cleanup is
/// the caller's responsibility (best-effort
/// `remove_dir_all` at end of test).
#[cfg(test)]
pub(crate) fn temp_scratch(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let dir = std::env::temp_dir()
        .join(format!("bellwether-xtask-{label}-{pid}-{seq}"));
    fs::create_dir_all(&dir).unwrap_or_else(|e| {
        panic!("failed to create scratch dir {}: {e}", dir.display())
    });
    dir
}

/// Resolve the cargo binary path. Prefers the `CARGO`
/// env var (set by cargo when running xtask) over a
/// PATH lookup.
pub fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

/// Resolve the workspace root (the parent of the xtask
/// crate directory).
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate always lives under workspace root")
        .to_path_buf()
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
    fn workspace_root_contains_cargo_toml() {
        let root = workspace_root();
        assert!(
            root.join("Cargo.toml").is_file(),
            "workspace root should contain Cargo.toml: {}",
            root.display()
        );
    }

    #[test]
    fn fmt_bytes_under_kib() {
        assert_eq!(fmt_bytes(0), "0 B");
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(1023), "1023 B");
    }

    #[test]
    fn fmt_bytes_scaling() {
        assert_eq!(fmt_bytes(1024), "1.0 KiB");
        assert_eq!(fmt_bytes(1536), "1.5 KiB");
        assert_eq!(fmt_bytes(1024 * 1024), "1.0 MiB");
        assert_eq!(fmt_bytes(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }

    #[test]
    fn elapsed_str_format() {
        let start = Instant::now();
        let result = elapsed_str(start);
        assert!(result.ends_with('s'), "should end with 's': {result}");
        assert!(result.contains('.'), "should have decimal: {result}");
    }
}
