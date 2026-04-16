use assert_cmd::Command;
use predicates::prelude::*;

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
