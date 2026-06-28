#![expect(clippy::unwrap_used, reason = "This is a test module")]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn skill_install_creates_files() {
    let temp = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.arg("--skill-install")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Skill installed to"));

    let skill_path = temp.path().join(".claude/skills/tixgraft/SKILL.md");
    assert!(skill_path.exists());

    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("tixgraft"));
}

#[test]
fn skill_uninstall_removes_directory() {
    let temp = TempDir::new().unwrap();

    // Install first
    Command::cargo_bin("tixgraft")
        .unwrap()
        .arg("--skill-install")
        .current_dir(temp.path())
        .assert()
        .success();

    let skill_dir = temp.path().join(".claude/skills/tixgraft");
    assert!(skill_dir.exists());

    // Uninstall
    Command::cargo_bin("tixgraft")
        .unwrap()
        .arg("--skill-uninstall")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Skill uninstalled from"));

    assert!(!skill_dir.exists());
}

#[test]
fn skill_uninstall_idempotent() {
    let temp = TempDir::new().unwrap();

    // Uninstall when nothing is installed
    Command::cargo_bin("tixgraft")
        .unwrap()
        .arg("--skill-uninstall")
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Skill was not installed"));
}

#[test]
fn skill_test_not_installed_auto_installs() {
    let temp = TempDir::new().unwrap();

    Command::cargo_bin("tixgraft")
        .unwrap()
        .args(["--skill-test", "--yes"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Skill installed to"));

    let skill_path = temp.path().join(".claude/skills/tixgraft/SKILL.md");
    assert!(skill_path.exists());
}

#[test]
fn skill_test_up_to_date() {
    let temp = TempDir::new().unwrap();

    // Install first
    Command::cargo_bin("tixgraft")
        .unwrap()
        .arg("--skill-install")
        .current_dir(temp.path())
        .assert()
        .success();

    // Test should report up to date
    Command::cargo_bin("tixgraft")
        .unwrap()
        .args(["--skill-test", "--yes"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("up to date"));
}

#[test]
fn skill_test_outdated_auto_upgrades() {
    let temp = TempDir::new().unwrap();

    // Install first
    Command::cargo_bin("tixgraft")
        .unwrap()
        .arg("--skill-install")
        .current_dir(temp.path())
        .assert()
        .success();

    // Modify the installed file
    let skill_path = temp.path().join(".claude/skills/tixgraft/SKILL.md");
    fs::write(&skill_path, "modified content").unwrap();

    // Test with --yes should auto-upgrade
    Command::cargo_bin("tixgraft")
        .unwrap()
        .args(["--skill-test", "--yes"])
        .current_dir(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Skill installed to"));

    // Verify content was restored
    let content = fs::read_to_string(&skill_path).unwrap();
    assert!(content.contains("tixgraft"));
    assert!(!content.contains("modified content"));
}

#[test]
fn skill_install_conflicts_with_skill_uninstall() {
    Command::cargo_bin("tixgraft")
        .unwrap()
        .args(["--skill-install", "--skill-uninstall"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn skill_install_conflicts_with_skill_test() {
    Command::cargo_bin("tixgraft")
        .unwrap()
        .args(["--skill-install", "--skill-test"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn skill_install_conflicts_with_to_command_line() {
    Command::cargo_bin("tixgraft")
        .unwrap()
        .args(["--skill-install", "--to-command-line"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn skill_install_conflicts_with_to_config() {
    Command::cargo_bin("tixgraft")
        .unwrap()
        .args(["--skill-install", "--to-config"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn global_flag_requires_skill_flag() {
    Command::cargo_bin("tixgraft")
        .unwrap()
        .arg("-g")
        .assert()
        .failure()
        .stdout(predicate::str::contains("--global (-g) requires"));
}

#[test]
fn yes_flag_requires_skill_test() {
    Command::cargo_bin("tixgraft")
        .unwrap()
        .args(["--yes", "--skill-install"])
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "--yes (-y) can only be used with --skill-test",
        ));
}
