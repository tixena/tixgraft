#![expect(clippy::unwrap_used, reason = "This is a test module")]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Create a local source directory with a single file and return its file:// URL.
fn create_local_source(
    temp_dir: &TempDir,
    dir_name: &str,
    file_name: &str,
    content: &str,
) -> String {
    let source_dir = temp_dir.path().join(dir_name);
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join(file_name), content).unwrap();
    let canonical = source_dir.canonicalize().unwrap();
    format!("file://{}", canonical.display())
}

// -- Config in different directory than cwd ------------------------------------

#[test]
fn target_resolves_relative_to_config_dir_not_cwd() {
    let temp_dir = TempDir::new().unwrap();

    // Create source repo
    let repo = create_local_source(&temp_dir, "repo", "hello.txt", "hello world");

    // Config lives in /project/ subdirectory
    let project_dir = temp_dir.path().join("project");
    fs::create_dir_all(&project_dir).unwrap();

    let config_content = format!(
        r#"
repository: "{repo}"
pulls:
  - source: "hello.txt"
    target: "./output/hello.txt"
    type: "file"
"#,
    );
    fs::write(project_dir.join("tixgraft.yaml"), &config_content).unwrap();

    // Run from the TEMP ROOT (not the project dir) with --config pointing
    // to the config in a subdirectory.
    let config_path = project_dir.join("tixgraft.yaml");

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg(config_path.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Completed pull operations successfully",
        ));

    // Target should resolve relative to config dir (/project/output/hello.txt),
    // NOT relative to cwd (/output/hello.txt).
    let expected_path = project_dir.join("output/hello.txt");
    assert!(
        expected_path.exists(),
        "File should land at {} (relative to config dir)",
        expected_path.display()
    );
    assert_eq!(fs::read_to_string(&expected_path).unwrap(), "hello world");

    // Should NOT appear in the cwd
    let wrong_path = temp_dir.path().join("output/hello.txt");
    assert!(
        !wrong_path.exists(),
        "File should NOT land at {} (relative to cwd)",
        wrong_path.display()
    );
}

#[test]
fn target_resolves_directory_pull_relative_to_config_dir() {
    let temp_dir = TempDir::new().unwrap();

    // Create source repo with a directory containing two files
    let source_dir = temp_dir.path().join("repo/templates");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("a.txt"), "file A").unwrap();
    fs::write(source_dir.join("b.txt"), "file B").unwrap();
    let canonical = temp_dir.path().join("repo").canonicalize().unwrap();
    let repo = format!("file://{}", canonical.display());

    // Config lives in /configs/ subdirectory
    let configs_dir = temp_dir.path().join("configs");
    fs::create_dir_all(&configs_dir).unwrap();

    let config_content = format!(
        r#"
repository: "{repo}"
pulls:
  - source: "templates"
    target: "./my_templates"
    type: "directory"
"#,
    );
    fs::write(configs_dir.join("tixgraft.yaml"), &config_content).unwrap();

    // Run from temp root, config in subdirectory
    let config_path = configs_dir.join("tixgraft.yaml");

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg(config_path.to_str().unwrap())
        .assert()
        .success();

    // Files should land under configs/my_templates/
    let expected_a = configs_dir.join("my_templates/a.txt");
    let expected_b = configs_dir.join("my_templates/b.txt");
    assert!(
        expected_a.exists(),
        "a.txt should land at {} (relative to config dir)",
        expected_a.display()
    );
    assert!(
        expected_b.exists(),
        "b.txt should land at {} (relative to config dir)",
        expected_b.display()
    );
    assert_eq!(fs::read_to_string(&expected_a).unwrap(), "file A");
    assert_eq!(fs::read_to_string(&expected_b).unwrap(), "file B");

    // Should NOT appear in cwd
    assert!(
        !temp_dir.path().join("my_templates").exists(),
        "Directory should NOT land in cwd"
    );
}

#[test]
fn auto_detected_config_resolves_to_cwd() {
    let temp_dir = TempDir::new().unwrap();

    // Create source repo
    let repo = create_local_source(&temp_dir, "repo", "auto.txt", "auto-detected config");

    // Config lives in the directory we'll use as cwd (auto-detection)
    let config_content = format!(
        r#"
repository: "{repo}"
pulls:
  - source: "auto.txt"
    target: "./auto_output/auto.txt"
    type: "file"
"#,
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), &config_content).unwrap();

    // Run from the same directory (auto-detection), no --config needed
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path()).assert().success();

    // Target should resolve relative to cwd (which is also the config dir)
    let expected = temp_dir.path().join("auto_output/auto.txt");
    assert!(
        expected.exists(),
        "File should land relative to cwd/config dir at {}",
        expected.display()
    );
    assert_eq!(
        fs::read_to_string(&expected).unwrap(),
        "auto-detected config"
    );
}

#[test]
fn children_and_parent_both_resolve_relative_to_own_config_dir() {
    let temp_dir = TempDir::new().unwrap();

    // Create sources
    let parent_repo = create_local_source(&temp_dir, "repo_p", "p.txt", "parent file");
    let child_repo = create_local_source(&temp_dir, "repo_c", "c.txt", "child file");

    // Child config in /project/sub/
    let child_dir = temp_dir.path().join("project/sub");
    fs::create_dir_all(&child_dir).unwrap();
    let child_config = format!(
        r#"
repository: "{child_repo}"
pulls:
  - source: "c.txt"
    target: "./child_out/c.txt"
    type: "file"
"#,
    );
    fs::write(child_dir.join("tixgraft.yaml"), child_config).unwrap();

    // Parent config in /project/
    let project_dir = temp_dir.path().join("project");
    let parent_config = format!(
        r#"
repository: "{parent_repo}"
pulls:
  - source: "p.txt"
    target: "./parent_out/p.txt"
    type: "file"
children:
  - "./sub/tixgraft.yaml"
"#,
    );
    fs::write(project_dir.join("tixgraft.yaml"), &parent_config).unwrap();

    // Run from TEMP ROOT (different from both config dirs)
    let config_path = project_dir.join("tixgraft.yaml");

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg(config_path.to_str().unwrap())
        .assert()
        .success();

    // Parent target resolves relative to /project/
    let parent_out = project_dir.join("parent_out/p.txt");
    assert!(
        parent_out.exists(),
        "Parent output should land at {} (relative to parent config dir)",
        parent_out.display()
    );
    assert_eq!(fs::read_to_string(&parent_out).unwrap(), "parent file");

    // Child target resolves relative to /project/sub/
    let child_out = child_dir.join("child_out/c.txt");
    assert!(
        child_out.exists(),
        "Child output should land at {} (relative to child config dir)",
        child_out.display()
    );
    assert_eq!(fs::read_to_string(&child_out).unwrap(), "child file");

    // Neither should land in the cwd
    assert!(
        !temp_dir.path().join("parent_out").exists(),
        "Parent output should NOT land in cwd"
    );
    assert!(
        !temp_dir.path().join("child_out").exists(),
        "Child output should NOT land in cwd"
    );
}

#[test]
fn commands_run_in_resolved_target_directory() {
    let temp_dir = TempDir::new().unwrap();

    // Create source repo with a directory
    let source_dir = temp_dir.path().join("repo/component");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("file.txt"), "component file").unwrap();
    let canonical = temp_dir.path().join("repo").canonicalize().unwrap();
    let repo = format!("file://{}", canonical.display());

    // Config in a subdirectory
    let config_dir = temp_dir.path().join("project");
    fs::create_dir_all(&config_dir).unwrap();

    let config_content = format!(
        r#"
repository: "{repo}"
pulls:
  - source: "component"
    target: "./comp_out"
    type: "directory"
    commands:
      - "pwd > pwd_output.txt"
"#,
    );
    fs::write(config_dir.join("tixgraft.yaml"), &config_content).unwrap();

    // Run from temp root
    let config_path = config_dir.join("tixgraft.yaml");

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg(config_path.to_str().unwrap())
        .assert()
        .success();

    // The command "pwd > pwd_output.txt" should run in the resolved target dir
    let pwd_file = config_dir.join("comp_out/pwd_output.txt");
    assert!(
        pwd_file.exists(),
        "pwd output should exist at {} (command ran in resolved target dir)",
        pwd_file.display()
    );

    // The pwd output should contain the resolved target path
    let pwd_content = fs::read_to_string(&pwd_file).unwrap();
    let resolved_target = config_dir.join("comp_out");
    let resolved_canonical = resolved_target.canonicalize().unwrap();
    assert!(
        pwd_content.trim() == resolved_canonical.to_str().unwrap(),
        "pwd should be {} but was {}",
        resolved_canonical.display(),
        pwd_content.trim()
    );
}

#[test]
fn replacements_applied_in_resolved_target() {
    let temp_dir = TempDir::new().unwrap();

    // Create source repo with a file containing a placeholder
    let repo = create_local_source(
        &temp_dir,
        "repo",
        "template.txt",
        "Hello {{NAME}}, welcome!",
    );

    // Config in a subdirectory
    let config_dir = temp_dir.path().join("project");
    fs::create_dir_all(&config_dir).unwrap();

    let config_content = format!(
        r#"
repository: "{repo}"
pulls:
  - source: "template.txt"
    target: "./out/template.txt"
    type: "file"
    replacements:
      - source: "{{{{NAME}}}}"
        target: "World"
"#,
    );
    fs::write(config_dir.join("tixgraft.yaml"), &config_content).unwrap();

    // Run from temp root
    let config_path = config_dir.join("tixgraft.yaml");

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg(config_path.to_str().unwrap())
        .assert()
        .success();

    // File with replacements should land in resolved target
    let output = config_dir.join("out/template.txt");
    assert!(
        output.exists(),
        "Template should land at {}",
        output.display()
    );
    assert_eq!(
        fs::read_to_string(&output).unwrap(),
        "Hello World, welcome!"
    );
}

#[test]
fn dry_run_shows_resolved_targets() {
    let temp_dir = TempDir::new().unwrap();

    // Create source repo
    let repo = create_local_source(&temp_dir, "repo", "dry.txt", "dry run content");

    // Config in a subdirectory
    let config_dir = temp_dir.path().join("project");
    fs::create_dir_all(&config_dir).unwrap();

    let config_content = format!(
        r#"
repository: "{repo}"
pulls:
  - source: "dry.txt"
    target: "./dry_output/dry.txt"
    type: "file"
"#,
    );
    fs::write(config_dir.join("tixgraft.yaml"), &config_content).unwrap();

    // Run with --dry-run from temp root
    let config_path = config_dir.join("tixgraft.yaml");

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    let output = cmd
        .current_dir(temp_dir.path())
        .arg("--config")
        .arg(config_path.to_str().unwrap())
        .arg("--dry-run")
        .output()
        .unwrap();

    assert!(output.status.success(), "Dry run should succeed");

    // The dry run output should show a target path that includes the config
    // directory rather than being relative to cwd.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let config_dir_str = config_dir.to_string_lossy();
    assert!(
        stdout.contains(config_dir_str.as_ref()),
        "Dry run should show target path containing config dir '{config_dir_str}' in output:\n{stdout}",
    );
}
