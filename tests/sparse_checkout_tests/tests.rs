//! Tests that run the binary against a temporary git repository with nested directories.
//!
//! They cover one nested path, several nested paths and a path that does not exist.

#![expect(clippy::unwrap_used, reason = "This is a test module")]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::TempDir;

/// Creates a repository on `master` with three nested directories of one YAML file each.
fn create_test_git_repo() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    StdCommand::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();

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

    StdCommand::new("git")
        .args(["branch", "-M", "master"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    temp_dir
}

#[test]
fn sparse_checkout_nested_path() {
    let repo_dir = create_test_git_repo();
    let work_dir = TempDir::new().unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    let assert_result = cmd
        .current_dir(work_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .arg("--verbose")
        .assert();

    assert_result.success();

    let output_file = work_dir
        .path()
        .join("output/percona-mongodb/deployment.yaml");
    assert!(
        output_file.exists(),
        "Expected file at {output_file:?} does not exist"
    );

    let content = fs::read_to_string(output_file).unwrap();
    assert!(content.contains("Deployment"));
}

#[test]
fn sparse_checkout_multiple_nested_paths() {
    let repo_dir = create_test_git_repo();
    let work_dir = TempDir::new().unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    let assert_result = cmd
        .current_dir(work_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .arg("--verbose")
        .assert();

    assert_result.success();

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
fn sparse_checkout_nonexistent_path() {
    let repo_dir = create_test_git_repo();
    let work_dir = TempDir::new().unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    let assert_result = cmd
        .current_dir(work_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .arg("--verbose")
        .assert();

    assert_result
        .failure()
        .stdout(predicate::str::contains("not found"));
}
