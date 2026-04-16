use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test-data")
        .join(name)
}

#[test]
fn cli_runs_successfully() {
    Command::cargo_bin("bellwether")
        .unwrap()
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello from bellwether"));
}

#[test]
fn cli_verbose_flag() {
    Command::cargo_bin("bellwether")
        .unwrap()
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("verbose mode enabled"));
}

#[test]
fn cli_version_flag() {
    Command::cargo_bin("bellwether")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("bellwether"));
}

#[test]
fn cli_help_flag() {
    Command::cargo_bin("bellwether")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage"));
}

#[test]
fn cli_loads_byos_config() {
    Command::cargo_bin("bellwether")
        .unwrap()
        .args(["--config", fixture("config-byos.toml").to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("trmnl mode = byos"))
        .stdout(predicate::str::contains("800x480"));
}

#[test]
fn cli_reports_error_for_missing_config() {
    let tmp = tempfile::TempDir::new().unwrap();
    let missing = tmp.path().join("definitely-not-a-real-path.toml");
    Command::cargo_bin("bellwether")
        .unwrap()
        .args(["--config", missing.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("loading config from"));
}
