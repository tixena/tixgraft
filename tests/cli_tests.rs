//! CLI interface tests

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_version_flag() {
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("tixgraft"));
}

#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "A CLI tool for fetching reusable components",
        ));
}

#[test]
fn test_missing_config_error() {
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.arg("--config")
        .arg("nonexistent.yaml")
        .assert()
        .failure()
        .code(1) // Configuration error
        .stdout(predicate::str::contains("Configuration file not found"));
}

#[test]
fn test_dry_run_with_example_config() {
    // Create a temporary config file
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");

    let config_content = r#"
repository: "example/test"
pulls:
  - source: "test/dir"
    target: "./output"
    type: "directory"
"#;

    fs::write(&config_path, config_content).unwrap();

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.arg("--config")
        .arg(config_path.to_str().unwrap())
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run preview"));
}

#[test]
fn test_invalid_yaml_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("invalid.yaml");

    // Invalid YAML content
    let invalid_yaml = r#"
repository: "example/test"
pulls:
  - source: "test/dir"
    target: "./output"
    invalid_field: [
"#;

    fs::write(&config_path, invalid_yaml).unwrap();

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.arg("--config")
        .arg(config_path.to_str().unwrap())
        .assert()
        .failure()
        .code(1) // Configuration error
        .stdout(predicate::str::contains("Failed to parse YAML"));
}

#[test]
fn test_config_validation_missing_pulls() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("no_pulls.yaml");

    let config_content = r#"
repository: "example/test"
tag: "main"
# Missing required pulls section
"#;

    fs::write(&config_path, config_content).unwrap();

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.arg("--config")
        .arg(config_path.to_str().unwrap())
        .assert()
        .failure()
        .code(1); // Configuration error - missing required field detected during parsing
}

#[test]
fn test_cli_args_minimal() {
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.arg("--repository")
        .arg("test/repo")
        .arg("--pull-source")
        .arg("src/dir")
        .arg("--pull-target")
        .arg("./target")
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run preview"));
}

#[test]
fn test_cli_args_mismatch_sources_targets() {
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.arg("--repository")
        .arg("test/repo")
        .arg("--pull-source")
        .arg("src/dir1")
        .arg("--pull-source")
        .arg("src/dir2")
        .arg("--pull-target")
        .arg("./target1")
        // Missing second target
        .assert()
        .failure()
        .code(1) // Configuration error
        .stdout(predicate::str::contains("Mismatch"));
}
