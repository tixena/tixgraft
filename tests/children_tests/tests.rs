#![expect(clippy::unwrap_used, reason = "This is a test module")]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Create a local source directory with a single file and return its canonicalized path.
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

// ── Basic execution ────────────────────────────────────────────────

#[test]
fn children_basic_execution() {
    let temp_dir = TempDir::new().unwrap();

    // Create two local sources: one for parent, one for child
    let parent_repo = create_local_source(&temp_dir, "repo_parent", "parent.txt", "parent content");
    let child_repo = create_local_source(&temp_dir, "repo_child", "child.txt", "child content");

    // Create child config in a subdirectory
    let child_dir = temp_dir.path().join("sub");
    fs::create_dir_all(&child_dir).unwrap();
    let child_config = format!(
        r#"
repository: "{child_repo}"
pulls:
  - source: "child.txt"
    target: "./child_out/child.txt"
    type: "file"
"#,
    );
    fs::write(child_dir.join("tixgraft.yaml"), child_config).unwrap();

    // Create parent config referencing the child
    let parent_config = format!(
        r#"
repository: "{parent_repo}"
pulls:
  - source: "parent.txt"
    target: "./parent_out/parent.txt"
    type: "file"
children:
  - "./sub/tixgraft.yaml"
"#,
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), parent_config).unwrap();

    // Execute
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Completed pull operations successfully",
        ));

    // Verify parent pull landed
    let parent_output = temp_dir.path().join("parent_out/parent.txt");
    assert!(parent_output.exists(), "Parent output file should exist");
    assert_eq!(
        fs::read_to_string(&parent_output).unwrap(),
        "parent content"
    );

    // Verify child pull landed (in the child's subdirectory)
    let child_output = temp_dir.path().join("sub/child_out/child.txt");
    assert!(child_output.exists(), "Child output file should exist");
    assert_eq!(fs::read_to_string(&child_output).unwrap(), "child content");
}

// ── CWD resolution ─────────────────────────────────────────────────

#[test]
fn children_cwd_resolution() {
    let temp_dir = TempDir::new().unwrap();

    // Create source for the child
    let child_repo = create_local_source(&temp_dir, "repo", "data.txt", "resolved data");

    // Create deeply nested child config
    let nested_dir = temp_dir.path().join("level1/level2");
    fs::create_dir_all(&nested_dir).unwrap();
    let child_config = format!(
        r#"
repository: "{child_repo}"
pulls:
  - source: "data.txt"
    target: "./out/data.txt"
    type: "file"
"#,
    );
    fs::write(nested_dir.join("tixgraft.yaml"), child_config).unwrap();

    // Parent config has only children, no pulls
    let parent_config = r#"
children:
  - "./level1/level2/tixgraft.yaml"
"#;
    fs::write(temp_dir.path().join("tixgraft.yaml"), parent_config).unwrap();

    // Execute
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // The child's target "./out/data.txt" should resolve relative to the child dir
    let output = temp_dir.path().join("level1/level2/out/data.txt");
    assert!(
        output.exists(),
        "File should land relative to child config dir at {}",
        output.display()
    );
    assert_eq!(fs::read_to_string(&output).unwrap(), "resolved data");

    // Should NOT appear at the parent level
    assert!(
        !temp_dir.path().join("out/data.txt").exists(),
        "File should not land in parent directory"
    );
}

// ── Default order: parent pulls first, then children ───────────────

#[test]
fn children_order_default() {
    let temp_dir = TempDir::new().unwrap();

    // Create sources
    let parent_repo = create_local_source(&temp_dir, "repo_parent", "marker.txt", "PARENT_MARKER");
    let child_repo = create_local_source(&temp_dir, "repo_child", "marker.txt", "CHILD_MARKER");

    // Child config
    let child_dir = temp_dir.path().join("child");
    fs::create_dir_all(&child_dir).unwrap();
    let child_config = format!(
        r#"
repository: "{child_repo}"
pulls:
  - source: "marker.txt"
    target: "./out/marker.txt"
    type: "file"
"#,
    );
    fs::write(child_dir.join("tixgraft.yaml"), child_config).unwrap();

    // Parent config (default order: pulls first, then children)
    let parent_config = format!(
        r#"
repository: "{parent_repo}"
pulls:
  - source: "marker.txt"
    target: "./parent_out/marker.txt"
    type: "file"
children:
  - "./child/tixgraft.yaml"
"#,
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), parent_config).unwrap();

    // Execute and capture output
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    let output = cmd
        .current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .output()
        .unwrap();

    assert!(output.status.success(), "Command should succeed");

    // Both files should exist
    assert!(temp_dir.path().join("parent_out/marker.txt").exists());
    assert!(temp_dir.path().join("child/out/marker.txt").exists());

    // In default order, parent's "Starting tixgraft" should appear before
    // "Processing child config" in the output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let pull_pos = stdout.find("Starting tixgraft pull operation");
    let child_pos = stdout.find("Processing child config");

    assert!(pull_pos.is_some(), "Should contain parent pull output");
    assert!(
        child_pos.is_some(),
        "Should contain child processing output"
    );
    assert!(
        pull_pos.unwrap() < child_pos.unwrap(),
        "Default order: parent pulls should execute before children"
    );
}

// ── processChildrenFirst reverses order ────────────────────────────

#[test]
fn children_order_process_children_first() {
    let temp_dir = TempDir::new().unwrap();

    // Create sources
    let parent_repo = create_local_source(&temp_dir, "repo_parent", "p.txt", "parent");
    let child_repo = create_local_source(&temp_dir, "repo_child", "c.txt", "child");

    // Child config
    let child_dir = temp_dir.path().join("child");
    fs::create_dir_all(&child_dir).unwrap();
    let child_config = format!(
        r#"
repository: "{child_repo}"
pulls:
  - source: "c.txt"
    target: "./out/c.txt"
    type: "file"
"#,
    );
    fs::write(child_dir.join("tixgraft.yaml"), child_config).unwrap();

    // Parent config with processChildrenFirst: true
    let parent_config = format!(
        r#"
repository: "{parent_repo}"
processChildrenFirst: true
pulls:
  - source: "p.txt"
    target: "./parent_out/p.txt"
    type: "file"
children:
  - "./child/tixgraft.yaml"
"#,
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), parent_config).unwrap();

    // Execute and capture output
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    let output = cmd
        .current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .output()
        .unwrap();

    assert!(output.status.success(), "Command should succeed");

    // Both files should exist
    assert!(temp_dir.path().join("parent_out/p.txt").exists());
    assert!(temp_dir.path().join("child/out/c.txt").exists());

    // With processChildrenFirst, child processing should appear before parent pulls
    let stdout = String::from_utf8_lossy(&output.stdout);
    let child_pos = stdout.find("Processing child config");
    let pull_pos = stdout.find("Starting tixgraft pull operation");

    assert!(
        child_pos.is_some(),
        "Should contain child processing output"
    );
    assert!(pull_pos.is_some(), "Should contain parent pull output");
    assert!(
        child_pos.unwrap() < pull_pos.unwrap(),
        "processChildrenFirst: children should execute before parent pulls"
    );
}

// ── Parent with only children (no pulls) ───────────────────────────

#[test]
fn children_only_no_pulls() {
    let temp_dir = TempDir::new().unwrap();

    // Create source for child
    let child_repo = create_local_source(&temp_dir, "repo", "file.txt", "from child repo");

    // Child config
    let child_dir = temp_dir.path().join("sub");
    fs::create_dir_all(&child_dir).unwrap();
    let child_config = format!(
        r#"
repository: "{child_repo}"
pulls:
  - source: "file.txt"
    target: "./output/file.txt"
    type: "file"
"#,
    );
    fs::write(child_dir.join("tixgraft.yaml"), child_config).unwrap();

    // Parent config with only children, no pulls
    let parent_config = r#"
children:
  - "./sub/tixgraft.yaml"
"#;
    fs::write(temp_dir.path().join("tixgraft.yaml"), parent_config).unwrap();

    // Execute
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify child pull executed
    let output = temp_dir.path().join("sub/output/file.txt");
    assert!(output.exists(), "Child output should exist");
    assert_eq!(fs::read_to_string(&output).unwrap(), "from child repo");
}

// ── True A → B → C recursive chain ─────────────────────────────

#[test]
fn children_nested() {
    let temp_dir = TempDir::new().unwrap();

    // Create separate sources for A, B, and C
    let repo_a = create_local_source(&temp_dir, "repo_a", "a.txt", "from A");
    let repo_b = create_local_source(&temp_dir, "repo_b", "b.txt", "from B");
    let repo_c = create_local_source(&temp_dir, "repo_c", "c.txt", "from C");

    // C config in level1/level2/
    let c_dir = temp_dir.path().join("level1/level2");
    fs::create_dir_all(&c_dir).unwrap();
    let c_config = format!(
        r#"
repository: "{repo_c}"
pulls:
  - source: "c.txt"
    target: "./out/c.txt"
    type: "file"
"#,
    );
    fs::write(c_dir.join("tixgraft.yaml"), c_config).unwrap();

    // B config in level1/, referencing C as a child
    let b_dir = temp_dir.path().join("level1");
    // b_dir already created by create_dir_all above
    let b_config = format!(
        r#"
repository: "{repo_b}"
pulls:
  - source: "b.txt"
    target: "./out/b.txt"
    type: "file"
children:
  - "./level2/tixgraft.yaml"
"#,
    );
    fs::write(b_dir.join("tixgraft.yaml"), b_config).unwrap();

    // A config (root), referencing B as a child
    let a_config = format!(
        r#"
repository: "{repo_a}"
pulls:
  - source: "a.txt"
    target: "./out/a.txt"
    type: "file"
children:
  - "./level1/tixgraft.yaml"
"#,
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), a_config).unwrap();

    // Execute
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify all three outputs exist at correct locations
    let a_output = temp_dir.path().join("out/a.txt");
    let b_output = temp_dir.path().join("level1/out/b.txt");
    let c_output = temp_dir.path().join("level1/level2/out/c.txt");

    assert!(a_output.exists(), "A output should exist");
    assert!(b_output.exists(), "B output should exist");
    assert!(c_output.exists(), "C output should exist");

    assert_eq!(fs::read_to_string(&a_output).unwrap(), "from A");
    assert_eq!(fs::read_to_string(&b_output).unwrap(), "from B");
    assert_eq!(fs::read_to_string(&c_output).unwrap(), "from C");
}

// ── Missing child config produces error ────────────────────────────

#[test]
fn children_missing_child_error() {
    let temp_dir = TempDir::new().unwrap();

    // Parent references a child that does not exist
    let parent_config = r#"
children:
  - "./nonexistent/tixgraft.yaml"
"#;
    fs::write(temp_dir.path().join("tixgraft.yaml"), parent_config).unwrap();

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .failure()
        .code(1_i32) // Configuration error
        .stdout(predicate::str::contains("Configuration validation failed"));
}

// ── Child pull failure propagates to parent ────────────────────────

#[test]
fn children_child_failure_propagates() {
    let temp_dir = TempDir::new().unwrap();

    // Create a source repo that does NOT contain the file the child expects
    let child_repo = create_local_source(&temp_dir, "repo_empty", "other.txt", "wrong file");

    // Child config references a file that doesn't exist in the repo
    let child_dir = temp_dir.path().join("sub");
    fs::create_dir_all(&child_dir).unwrap();
    let child_config = format!(
        r#"
repository: "{child_repo}"
pulls:
  - source: "missing_file.txt"
    target: "./out/missing.txt"
    type: "file"
"#,
    );
    fs::write(child_dir.join("tixgraft.yaml"), child_config).unwrap();

    // Parent config with only the failing child
    let parent_config = r#"
children:
  - "./sub/tixgraft.yaml"
"#;
    fs::write(temp_dir.path().join("tixgraft.yaml"), parent_config).unwrap();

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .failure()
        .code(2_i32) // Source error (file not found in repo)
        .stdout(predicate::str::contains("Error in child"));
}
