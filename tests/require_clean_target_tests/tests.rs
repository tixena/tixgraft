#![expect(clippy::unwrap_used, reason = "This is a test module")]

use os_shim::System as _;
use os_shim::real::RealSystem;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tixgraft::cli::PullConfig;

/// Initialize a git repo in the given directory with an initial commit.
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
    let config: PullConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(config.require_clean_target);
}

#[test]
fn serde_explicit_false() {
    let yaml = "source: src\ntarget: ./dst\nrequireCleanTarget: false\n";
    let config: PullConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(!config.require_clean_target);
}

#[test]
fn serde_explicit_true() {
    let yaml = "source: src\ntarget: ./dst\nrequireCleanTarget: true\n";
    let config: PullConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(config.require_clean_target);
}

#[test]
fn clean_target_passes() {
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().unwrap();
    let root = temp_dir.path();

    git_init(root);

    // Create a tracked, committed subdirectory
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

    // Dry run should succeed (target is clean)
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
    let temp_dir = system.create_temp_dir().unwrap();
    let root = temp_dir.path();

    git_init(root);

    // Create and commit, then modify
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

    // Should fail because target has uncommitted changes
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
    let temp_dir = system.create_temp_dir().unwrap();
    let root = temp_dir.path();

    git_init(root);

    // Create and commit, then dirty
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

    // Dry-run should succeed (check is disabled)
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

/// Write a config for a directory-type pull.
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
    let temp_dir = system.create_temp_dir().unwrap();
    let root = temp_dir.path();

    git_init(root);

    // Create a source directory in the repo
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

    // Create a target directory and commit it
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

    // Dirty the target (modify a committed file)
    fs::write(root.join("target_dir/hello.txt"), b"modified locally").unwrap();

    // Config with reset: true and requireCleanTarget: true (default)
    let config_path = write_dir_config(root, "src_dir", "./target_dir", "    reset: true\n");

    // Should fail: dirty target should be caught BEFORE reset
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
    let temp_dir = system.create_temp_dir().unwrap();
    let root = temp_dir.path();

    git_init(root);

    // Create a source directory in the repo
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

    // Create a target directory and commit it
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

    // Dirty the target
    fs::write(root.join("target_dir/hello.txt"), b"modified locally").unwrap();

    // Config with reset: true and requireCleanTarget: false (explicit opt-out)
    let config_path = write_dir_config(
        root,
        "src_dir",
        "./target_dir",
        "    reset: true\n    requireCleanTarget: false\n",
    );

    // Should succeed: clean check is explicitly disabled
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
    let temp_dir = system.create_temp_dir().unwrap();
    let root = temp_dir.path();

    git_init(root);

    // Create a source directory in the repo
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

    // Create target directory and commit it (clean state)
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

    // Config with reset: true and requireCleanTarget: true (default)
    let config_path = write_dir_config(root, "src_dir", "./target_dir", "    reset: true\n");

    // Should succeed: target is clean, reset is fine
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
    // This reproduces the exact bug scenario:
    // 1. First tixgraft run creates target with files
    // 2. User does NOT commit those files
    // 3. Second tixgraft run with reset: true should fail, not silently delete
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().unwrap();
    let root = temp_dir.path();

    git_init(root);

    // Create a source directory in the repo
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

    // Simulate first tixgraft run: create target with UNTRACKED files
    // (not committed, not staged)
    fs::create_dir_all(root.join("target_dir")).unwrap();
    fs::write(root.join("target_dir/hello.txt"), b"hello from graft").unwrap();

    // Config with reset: true and requireCleanTarget: true (default)
    let config_path = write_dir_config(root, "src_dir", "./target_dir", "    reset: true\n");

    // Should fail: target has untracked (uncommitted) files
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
    // Files that are staged (git add) but not committed should also be caught
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().unwrap();
    let root = temp_dir.path();

    git_init(root);

    // Create source directory
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

    // Create target with files that are staged but not committed
    fs::create_dir_all(root.join("target_dir")).unwrap();
    fs::write(root.join("target_dir/hello.txt"), b"staged content").unwrap();
    Command::new("git")
        .args(["add", "target_dir/"])
        .current_dir(root)
        .output()
        .unwrap();

    // Config with reset: true, requireCleanTarget defaults to true
    let config_path = write_dir_config(root, "src_dir", "./target_dir", "    reset: true\n");

    // Should fail: staged but uncommitted files
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

#[test]
fn reset_true_double_run_without_commit_should_fail() {
    // Exact reproduction from bug report:
    // 1. Run tixgraft with reset: true (creates target with new files)
    // 2. Don't commit the pulled files
    // 3. Run tixgraft again
    // Expected: error about uncommitted changes
    // Bug: silently deletes and re-pulls
    let system = RealSystem::new();
    let temp_dir = system.create_temp_dir().unwrap();
    let root = temp_dir.path();

    git_init(root);

    // Create a source directory in the repo and commit it
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

    // Config with reset: true (requireCleanTarget defaults to true)
    let config_path = write_dir_config(root, "src_dir", "./target_dir", "    reset: true\n");
    // Need to commit the config so it doesn't show as dirty
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

    // First run: should succeed (target doesn't exist yet)
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

    // Verify target was created
    assert!(root.join("target_dir/hello.txt").exists());

    // Do NOT commit the pulled files -- they are untracked

    // Second run: should fail because target has uncommitted files
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
    let temp_dir = system.create_temp_dir().unwrap();
    let root = temp_dir.path();

    git_init(root);

    // Target doesn't exist — config points to ./target_dir which is absent
    let config_path = write_config(root, "");

    // Dry-run should succeed (target doesn't exist = clean)
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
