use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn cli_runs_successfully() {
    Command::cargo_bin("rustbase")
        .unwrap()
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello from rustbase"));
}

#[test]
fn cli_verbose_flag() {
    Command::cargo_bin("rustbase")
        .unwrap()
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("verbose mode enabled"));
}

#[test]
fn cli_version_flag() {
    Command::cargo_bin("rustbase")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("rustbase"));
}

#[test]
fn cli_help_flag() {
    Command::cargo_bin("rustbase")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage"));
}
