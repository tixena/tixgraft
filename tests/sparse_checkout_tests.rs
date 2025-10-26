//! Integration tests for Git sparse checkout functionality
//!
//! These tests verify that sparse checkout works correctly with nested directory
//! structures and that the temporary directory lifetime is managed properly.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::TempDir;

/// Helper to create a test Git repository with nested directory structure
fn create_test_git_repo() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repository
    StdCommand::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Configure git
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    StdCommand::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create nested directory structure similar to the user's issue
    fs::create_dir_all(repo_path.join("kubernetes/kustomize/infrastructure/percona-mongodb"))
        .unwrap();
    fs::write(
        repo_path.join("kubernetes/kustomize/infrastructure/percona-mongodb/deployment.yaml"),
        "apiVersion: v1\nkind: Deployment",
    )
    .unwrap();

    fs::create_dir_all(repo_path.join("kubernetes/kustomize/infrastructure/monitoring")).unwrap();
    fs::write(
        repo_path.join("kubernetes/kustomize/infrastructure/monitoring/config.yaml"),
        "apiVersion: v1\nkind: ConfigMap",
    )
    .unwrap();

    fs::create_dir_all(repo_path.join("kubernetes/kustomize/infrastructure/nginx-ingress"))
        .unwrap();
    fs::write(
        repo_path.join("kubernetes/kustomize/infrastructure/nginx-ingress/ingress.yaml"),
        "apiVersion: networking.k8s.io/v1\nkind: Ingress",
    )
    .unwrap();

    // Add and commit files
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    StdCommand::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create master branch explicitly
    StdCommand::new("git")
        .args(["branch", "-M", "master"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    temp_dir
}

#[test]
fn test_sparse_checkout_nested_path() {
    let repo_dir = create_test_git_repo();
    let work_dir = TempDir::new().unwrap();

    // Create config file pointing to the test repository
    let repo_abs = repo_dir.path().canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
tag: "master"
pulls:
  - source: "kubernetes/kustomize/infrastructure/percona-mongodb"
    target: "./output/percona-mongodb"
    type: "directory"
"#,
        repo_abs.display()
    );

    fs::write(work_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    let assert_result = cmd
        .current_dir(work_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .arg("--verbose")
        .assert();

    // Print output for debugging
    let output = assert_result.get_output();
    println!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
    println!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));

    // Check if it succeeded
    assert_result.success();

    // Verify the file was copied
    let output_file = work_dir
        .path()
        .join("output/percona-mongodb/deployment.yaml");
    assert!(
        output_file.exists(),
        "Expected file at {:?} does not exist",
        output_file
    );

    let content = fs::read_to_string(output_file).unwrap();
    assert!(content.contains("Deployment"));
}

#[test]
fn test_sparse_checkout_multiple_nested_paths() {
    let repo_dir = create_test_git_repo();
    let work_dir = TempDir::new().unwrap();

    // Create config file with multiple pulls (like the user's config)
    let repo_abs = repo_dir.path().canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
tag: "master"
pulls:
  - source: "kubernetes/kustomize/infrastructure/percona-mongodb"
    target: "./output/percona-mongodb"
    type: "directory"
  - source: "kubernetes/kustomize/infrastructure/monitoring"
    target: "./output/monitoring"
    type: "directory"
  - source: "kubernetes/kustomize/infrastructure/nginx-ingress"
    target: "./output/nginx-ingress"
    type: "directory"
"#,
        repo_abs.display()
    );

    fs::write(work_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    let assert_result = cmd
        .current_dir(work_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .arg("--verbose")
        .assert();

    // Print output for debugging
    let output = assert_result.get_output();
    println!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
    println!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));

    // Check if it succeeded
    assert_result.success();

    // Verify all files were copied
    assert!(
        work_dir
            .path()
            .join("output/percona-mongodb/deployment.yaml")
            .exists()
    );
    assert!(
        work_dir
            .path()
            .join("output/monitoring/config.yaml")
            .exists()
    );
    assert!(
        work_dir
            .path()
            .join("output/nginx-ingress/ingress.yaml")
            .exists()
    );
}

#[test]
fn test_sparse_checkout_nonexistent_path() {
    let repo_dir = create_test_git_repo();
    let work_dir = TempDir::new().unwrap();

    // Create config file with nonexistent path
    let repo_abs = repo_dir.path().canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
tag: "master"
pulls:
  - source: "nonexistent/path"
    target: "./output"
    type: "directory"
"#,
        repo_abs.display()
    );

    fs::write(work_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft - should fail
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    let assert_result = cmd
        .current_dir(work_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .arg("--verbose")
        .assert();

    // Print output for debugging
    let output = assert_result.get_output();
    println!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
    println!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));

    // Should fail with source not found error
    assert_result
        .failure()
        .stdout(predicate::str::contains("not found"));
}
