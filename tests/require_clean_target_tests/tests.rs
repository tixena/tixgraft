//! Tests that run the binary in real git repositories with a dirty, clean or absent target.
//!
//! They cover the default value, modified, untracked and staged files, and the effect on `reset`.

#![expect(clippy::unwrap_used, reason = "This is a test module")]

use os_shim::System as _;
use os_shim::real::RealSystem;
use pollster::FutureExt as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tixgraft::cli::PullConfig;

/// Creates a repository in `dir` and commits `init.txt`, the source `write_config` pulls.
fn git_init(dir: &Path) {
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .unwrap();
    fs::write(dir.join("init.txt"), b"init").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir)
        .output()
        .unwrap();
}

fn write_config(root: &Path, extra_pull_fields: &str) -> PathBuf {
    let config_yaml = format!(
        "repository: \"file:{root}\"\ntag: HEAD\npulls:\n  - source: init.txt\n    target: ./target_dir\n    type: file\n{extra_pull_fields}",
        root = root.display(),
    );
    let config_path = root.join("tixgraft.yaml");
    fs::write(&config_path, config_yaml).unwrap();
    config_path
}

#[test]
fn serde_default_is_true() {
    let yaml = "source: src\ntarget: ./dst\n";
    let config: PullConfig = serde_saphyr::from_str(yaml).unwrap();
    assert!(config.require_clean_target);
}

#[test]
fn serde_explicit_false() {
    let yaml = "source: src\ntarget: ./dst\nrequireCleanTarget: false\n";
    let config: PullConfig = serde_saphyr::from_str(yaml).unwrap();
    assert!(!config.require_clean_target);
}

#[test]
fn serde_explicit_true() {
    let yaml = "source: src\ntarget: ./dst\nrequireCleanTarget: true\n";
    let config: PullConfig = serde_saphyr::from_str(yaml).unwrap();
    assert!(config.require_clean_target);
}

#[test]
fn clean_target_passes() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let root = temp_dir.path();

    git_init(root);

    fs::create_dir_all(root.join("target_dir")).unwrap();
    fs::write(root.join("target_dir/file.txt"), b"committed").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add target"])
        .current_dir(root)
        .output()
        .unwrap();

    let config_path = write_config(root, "");

    let output = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap(), "--dry-run"])
        .current_dir(root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "dry-run should pass on clean target: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn dirty_target_fails() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let root = temp_dir.path();

    git_init(root);

    fs::create_dir_all(root.join("target_dir")).unwrap();
    fs::write(root.join("target_dir/file.txt"), b"committed").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add target"])
        .current_dir(root)
        .output()
        .unwrap();
    fs::write(root.join("target_dir/file.txt"), b"modified locally").unwrap();

    let config_path = write_config(root, "");

    let output = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap()])
        .current_dir(root)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let all_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        all_output.contains("uncommitted changes"),
        "should mention uncommitted changes: {all_output}"
    );
}

#[test]
fn dirty_target_passes_when_disabled() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let root = temp_dir.path();

    git_init(root);

    fs::create_dir_all(root.join("target_dir")).unwrap();
    fs::write(root.join("target_dir/file.txt"), b"committed").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add target"])
        .current_dir(root)
        .output()
        .unwrap();
    fs::write(root.join("target_dir/file.txt"), b"modified locally").unwrap();

    let config_path = write_config(root, "    requireCleanTarget: false\n");

    let output = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap(), "--dry-run"])
        .current_dir(root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "dry-run should pass when requireCleanTarget is false: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_dir_config(root: &Path, source: &str, target: &str, extra: &str) -> PathBuf {
    let config_yaml = format!(
        "repository: \"file:{root}\"\ntag: HEAD\npulls:\n  - source: {source}\n    target: {target}\n    type: directory\n{extra}",
        root = root.display(),
    );
    let config_path = root.join("tixgraft.yaml");
    fs::write(&config_path, config_yaml).unwrap();
    config_path
}

#[test]
fn reset_true_with_dirty_target_should_fail() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let root = temp_dir.path();

    git_init(root);

    fs::create_dir_all(root.join("src_dir")).unwrap();
    fs::write(root.join("src_dir/hello.txt"), b"hello").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add src_dir"])
        .current_dir(root)
        .output()
        .unwrap();

    fs::create_dir_all(root.join("target_dir")).unwrap();
    fs::write(root.join("target_dir/hello.txt"), b"hello").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add target_dir"])
        .current_dir(root)
        .output()
        .unwrap();

    fs::write(root.join("target_dir/hello.txt"), b"modified locally").unwrap();

    let config_path = write_dir_config(root, "src_dir", "./target_dir", "    reset: true\n");

    let output = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap()])
        .current_dir(root)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "reset: true should NOT bypass requireCleanTarget check.\n\
             stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let all_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        all_output.contains("uncommitted changes"),
        "should mention uncommitted changes: {all_output}"
    );
}

#[test]
fn reset_true_with_dirty_target_passes_when_clean_check_disabled() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let root = temp_dir.path();

    git_init(root);

    fs::create_dir_all(root.join("src_dir")).unwrap();
    fs::write(root.join("src_dir/hello.txt"), b"hello").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add src_dir"])
        .current_dir(root)
        .output()
        .unwrap();

    fs::create_dir_all(root.join("target_dir")).unwrap();
    fs::write(root.join("target_dir/hello.txt"), b"hello").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add target_dir"])
        .current_dir(root)
        .output()
        .unwrap();

    fs::write(root.join("target_dir/hello.txt"), b"modified locally").unwrap();

    let config_path = write_dir_config(
        root,
        "src_dir",
        "./target_dir",
        "    reset: true\n    requireCleanTarget: false\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap()])
        .current_dir(root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "reset: true + requireCleanTarget: false should succeed.\n\
             stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reset_true_with_clean_target_should_pass() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let root = temp_dir.path();

    git_init(root);

    fs::create_dir_all(root.join("src_dir")).unwrap();
    fs::write(root.join("src_dir/hello.txt"), b"hello").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add src_dir"])
        .current_dir(root)
        .output()
        .unwrap();

    fs::create_dir_all(root.join("target_dir")).unwrap();
    fs::write(root.join("target_dir/hello.txt"), b"hello").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add target_dir"])
        .current_dir(root)
        .output()
        .unwrap();

    let config_path = write_dir_config(root, "src_dir", "./target_dir", "    reset: true\n");

    let output = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap()])
        .current_dir(root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "reset: true with clean target should pass.\n\
             stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reset_true_with_untracked_target_files_should_fail() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let root = temp_dir.path();

    git_init(root);

    fs::create_dir_all(root.join("src_dir")).unwrap();
    fs::write(root.join("src_dir/hello.txt"), b"hello").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add src_dir"])
        .current_dir(root)
        .output()
        .unwrap();

    fs::create_dir_all(root.join("target_dir")).unwrap();
    fs::write(root.join("target_dir/hello.txt"), b"hello from graft").unwrap();

    let config_path = write_dir_config(root, "src_dir", "./target_dir", "    reset: true\n");

    let output = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap()])
        .current_dir(root)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "reset: true should NOT bypass requireCleanTarget check for untracked files.\n\
             stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let all_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        all_output.contains("uncommitted changes"),
        "should mention uncommitted changes: {all_output}"
    );
}

#[test]
fn reset_true_with_staged_target_files_should_fail() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let root = temp_dir.path();

    git_init(root);

    fs::create_dir_all(root.join("src_dir")).unwrap();
    fs::write(root.join("src_dir/hello.txt"), b"hello").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add src_dir"])
        .current_dir(root)
        .output()
        .unwrap();

    fs::create_dir_all(root.join("target_dir")).unwrap();
    fs::write(root.join("target_dir/hello.txt"), b"staged content").unwrap();
    Command::new("git")
        .args(["add", "target_dir/"])
        .current_dir(root)
        .output()
        .unwrap();

    let config_path = write_dir_config(root, "src_dir", "./target_dir", "    reset: true\n");

    let output = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap()])
        .current_dir(root)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "reset: true should NOT bypass requireCleanTarget check for staged files.\n\
             stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A second `reset: true` run fails on the files the first run pulled and nobody committed.
#[test]
fn reset_true_double_run_without_commit_should_fail() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let root = temp_dir.path();

    git_init(root);

    fs::create_dir_all(root.join("src_dir")).unwrap();
    fs::write(root.join("src_dir/hello.txt"), b"hello from template").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add src_dir"])
        .current_dir(root)
        .output()
        .unwrap();

    let config_path = write_dir_config(root, "src_dir", "./target_dir", "    reset: true\n");
    Command::new("git")
        .args(["add", "tixgraft.yaml"])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add config"])
        .current_dir(root)
        .output()
        .unwrap();

    let first_run = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap()])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        first_run.status.success(),
        "first run should succeed: {}",
        String::from_utf8_lossy(&first_run.stderr)
    );

    assert!(root.join("target_dir/hello.txt").exists());

    let output = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap()])
        .current_dir(root)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "second run with reset: true should fail when target has uncommitted files.\n\
             The requireCleanTarget check should run BEFORE reset.\n\
             stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let all_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        all_output.contains("uncommitted changes"),
        "should mention uncommitted changes: {all_output}"
    );
}

#[test]
fn nonexistent_target_passes() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let root = temp_dir.path();

    git_init(root);

    let config_path = write_config(root, "");

    let output = Command::new(env!("CARGO_BIN_EXE_tixgraft"))
        .args(["--config", config_path.to_str().unwrap(), "--dry-run"])
        .current_dir(root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "dry-run should pass when target doesn't exist: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
