//! Integration tests for local filesystem repository sources

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn local_file_source_with_file_prefix() {
    let temp_dir = TempDir::new().unwrap();

    // Create source directory structure
    fs::create_dir(temp_dir.path().join("source")).unwrap();
    fs::create_dir(temp_dir.path().join("source/templates")).unwrap();
    fs::write(
        temp_dir.path().join("source/templates/file.txt"),
        "Hello from local source",
    )
    .unwrap();

    // Create config file
    let source_abs = temp_dir.path().join("source").canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
pulls:
  - source: "templates/file.txt"
    target: "./target/output.txt"
    type: "file"
"#,
        source_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft from temp_dir
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Completed pull operations successfully",
        ));

    // Verify file was copied
    let output_file = temp_dir.path().join("target/output.txt");
    assert!(output_file.exists());
    let content = fs::read_to_string(output_file).unwrap();
    assert_eq!(content, "Hello from local source");
}

#[test]
fn local_directory_source_with_file_prefix() {
    let temp_dir = TempDir::new().unwrap();

    // Create source directory structure
    fs::create_dir(temp_dir.path().join("source")).unwrap();
    fs::create_dir(temp_dir.path().join("source/templates")).unwrap();
    fs::write(temp_dir.path().join("source/templates/file1.txt"), "File 1").unwrap();
    fs::write(temp_dir.path().join("source/templates/file2.txt"), "File 2").unwrap();
    fs::create_dir(temp_dir.path().join("source/templates/subdir")).unwrap();
    fs::write(
        temp_dir.path().join("source/templates/subdir/file3.txt"),
        "File 3",
    )
    .unwrap();

    // Create config file
    let source_abs = temp_dir.path().join("source").canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
pulls:
  - source: "templates"
    target: "./target/output"
    type: "directory"
"#,
        source_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify directory was copied
    let output_dir = temp_dir.path().join("target/output");
    assert!(output_dir.exists());
    assert!(output_dir.join("file1.txt").exists());
    assert!(output_dir.join("file2.txt").exists());
    assert!(output_dir.join("subdir/file3.txt").exists());

    assert_eq!(
        fs::read_to_string(output_dir.join("file1.txt")).unwrap(),
        "File 1"
    );
    assert_eq!(
        fs::read_to_string(output_dir.join("file2.txt")).unwrap(),
        "File 2"
    );
    assert_eq!(
        fs::read_to_string(output_dir.join("subdir/file3.txt")).unwrap(),
        "File 3"
    );
}

#[test]
fn local_source_with_absolute_path() {
    let temp_dir = TempDir::new().unwrap();

    // Create source directory structure
    fs::create_dir(temp_dir.path().join("source")).unwrap();
    fs::write(temp_dir.path().join("source/file.txt"), "Content").unwrap();

    // Create config file using absolute path with file: prefix
    let source_abs = temp_dir.path().join("source").canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file:{}"
pulls:
  - source: "file.txt"
    target: "./target/output.txt"
    type: "file"
"#,
        source_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify file was copied
    let output_file = temp_dir.path().join("target/output.txt");
    assert!(output_file.exists());
    assert_eq!(fs::read_to_string(output_file).unwrap(), "Content");
}

#[test]
fn local_source_with_relative_path() {
    let temp_dir = TempDir::new().unwrap();

    // Create source directory structure
    fs::create_dir(temp_dir.path().join("source")).unwrap();
    fs::write(temp_dir.path().join("source/file.txt"), "Relative content").unwrap();

    // Create config file using relative path with file: prefix
    let config = r#"
repository: "file:./source"
pulls:
  - source: "file.txt"
    target: "./target/output.txt"
    type: "file"
"#;
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft from temp_dir
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify file was copied
    let output_file = temp_dir.path().join("target/output.txt");
    assert!(output_file.exists());
    assert_eq!(fs::read_to_string(output_file).unwrap(), "Relative content");
}

#[test]
fn local_source_with_replacements() {
    let temp_dir = TempDir::new().unwrap();

    // Create source file with placeholders
    fs::create_dir(temp_dir.path().join("source")).unwrap();
    fs::write(
        temp_dir.path().join("source/template.txt"),
        "Hello {{NAME}}, your project is {{PROJECT}}!",
    )
    .unwrap();

    // Create config file
    let source_abs = temp_dir.path().join("source").canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
pulls:
  - source: "template.txt"
    target: "./target/output.txt"
    type: "file"
    replacements:
      - source: "{{{{NAME}}}}"
        target: "Alice"
      - source: "{{{{PROJECT}}}}"
        target: "TixGraft"
"#,
        source_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify replacements were applied
    let output_file = temp_dir.path().join("target/output.txt");
    assert!(output_file.exists());
    let content = fs::read_to_string(output_file).unwrap();
    assert_eq!(content, "Hello Alice, your project is TixGraft!");
}

#[test]
fn local_source_with_env_var_replacement() {
    let temp_dir = TempDir::new().unwrap();

    // Create source file with placeholders
    fs::create_dir(temp_dir.path().join("source")).unwrap();
    fs::write(
        temp_dir.path().join("source/template.txt"),
        "App: {{APP_NAME}}",
    )
    .unwrap();

    // Create config file
    let source_abs = temp_dir.path().join("source").canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
pulls:
  - source: "template.txt"
    target: "./target/output.txt"
    type: "file"
    replacements:
      - source: "{{{{APP_NAME}}}}"
        valueFromEnv: "TEST_APP_NAME"
"#,
        source_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft with environment variable set for the subprocess
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .env("TEST_APP_NAME", "MyApp") // Set env var for subprocess only
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify replacements were applied
    let output_file = temp_dir.path().join("target/output.txt");
    assert!(output_file.exists());
    let content = fs::read_to_string(output_file).unwrap();
    assert_eq!(content, "App: MyApp");
}

#[test]
fn local_source_with_commands() {
    let temp_dir = TempDir::new().unwrap();

    // Create source file
    fs::create_dir(temp_dir.path().join("source")).unwrap();
    fs::write(temp_dir.path().join("source/file.txt"), "Original").unwrap();

    // Create config file with post-copy command
    let source_abs = temp_dir.path().join("source").canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
pulls:
  - source: "file.txt"
    target: "./target/file.txt"
    type: "file"
    commands:
      - "echo 'Command executed' > command_output.txt"
"#,
        source_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify file was copied
    assert!(temp_dir.path().join("target/file.txt").exists());

    // Verify command was executed (commands run in target directory - parent of target file)
    let command_output = temp_dir.path().join("target/command_output.txt");
    assert!(command_output.exists());
    let content = fs::read_to_string(command_output).unwrap();
    assert!(content.contains("Command executed"));
}

#[test]
fn local_source_with_reset() {
    let temp_dir = TempDir::new().unwrap();

    // Create source file
    fs::create_dir(temp_dir.path().join("source")).unwrap();
    fs::write(temp_dir.path().join("source/new.txt"), "New content").unwrap();

    // Create target with existing file
    fs::create_dir(temp_dir.path().join("target")).unwrap();
    fs::write(temp_dir.path().join("target/old.txt"), "Old content").unwrap();

    // Create config file with reset: true for directory
    let source_abs = temp_dir.path().join("source").canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
pulls:
  - source: "."
    target: "./target"
    type: "directory"
    reset: true
"#,
        source_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify new file exists and old file was removed
    assert!(temp_dir.path().join("target/new.txt").exists());
    assert!(!temp_dir.path().join("target/old.txt").exists());
}

#[test]
fn local_source_nonexistent_path() {
    let temp_dir = TempDir::new().unwrap();

    // Create config file with nonexistent local path
    let config = r#"
repository: "file:///nonexistent/path/to/repo"
pulls:
  - source: "file.txt"
    target: "./output.txt"
    type: "file"
"#;
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft - should fail
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .failure();
    // Should fail - accept any error since it's about a nonexistent path
}

#[test]
fn local_source_missing_file() {
    let temp_dir = TempDir::new().unwrap();

    // Create source directory but not the file
    fs::create_dir(temp_dir.path().join("source")).unwrap();

    // Create config file
    let source_abs = temp_dir.path().join("source").canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
pulls:
  - source: "missing.txt"
    target: "./target/output.txt"
    type: "file"
"#,
        source_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft - should fail
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .failure()
        .stdout(predicate::str::contains("not found"));
}

#[test]
fn mixed_local_sources() {
    let temp_dir = TempDir::new().unwrap();

    // Create two local sources
    fs::create_dir(temp_dir.path().join("source1")).unwrap();
    fs::write(temp_dir.path().join("source1/file1.txt"), "From source 1").unwrap();

    fs::create_dir(temp_dir.path().join("source2")).unwrap();
    fs::write(temp_dir.path().join("source2/file2.txt"), "From source 2").unwrap();

    // Create config file with multiple local sources
    let source1_abs = temp_dir.path().join("source1").canonicalize().unwrap();
    let source2_abs = temp_dir.path().join("source2").canonicalize().unwrap();
    let config = format!(
        r#"
pulls:
  - source: "file1.txt"
    target: "./target/file1.txt"
    type: "file"
    repository: "file://{}"
  - source: "file2.txt"
    target: "./target/file2.txt"
    type: "file"
    repository: "file://{}"
"#,
        source1_abs.display(),
        source2_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify both files were copied
    assert!(temp_dir.path().join("target/file1.txt").exists());
    assert!(temp_dir.path().join("target/file2.txt").exists());
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("target/file1.txt")).unwrap(),
        "From source 1"
    );
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("target/file2.txt")).unwrap(),
        "From source 2"
    );
}

#[test]
fn local_source_dry_run() {
    let temp_dir = TempDir::new().unwrap();

    // Create source file
    fs::create_dir(temp_dir.path().join("source")).unwrap();
    fs::write(temp_dir.path().join("source/file.txt"), "Content").unwrap();

    // Create config file
    let source_abs = temp_dir.path().join("source").canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
pulls:
  - source: "file.txt"
    target: "./target/output.txt"
    type: "file"
"#,
        source_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft with dry-run
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run preview"));

    // Verify file was NOT copied
    assert!(!temp_dir.path().join("target").exists());
}

#[test]
fn local_source_per_pull_override() {
    let temp_dir = TempDir::new().unwrap();

    // Create two different source directories
    fs::create_dir(temp_dir.path().join("source1")).unwrap();
    fs::write(temp_dir.path().join("source1/file.txt"), "From source 1").unwrap();

    fs::create_dir(temp_dir.path().join("source2")).unwrap();
    fs::write(temp_dir.path().join("source2/file.txt"), "From source 2").unwrap();

    // Create config file with global repository and per-pull override
    let source1_abs = temp_dir.path().join("source1").canonicalize().unwrap();
    let source2_abs = temp_dir.path().join("source2").canonicalize().unwrap();
    let config = format!(
        r#"
repository: "file://{}"
pulls:
  - source: "file.txt"
    target: "./target/output1.txt"
    type: "file"
  - source: "file.txt"
    target: "./target/output2.txt"
    type: "file"
    repository: "file://{}"
"#,
        source1_abs.display(),
        source2_abs.display()
    );
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    // Run tixgraft
    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    // Verify files were copied from correct sources
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("target/output1.txt")).unwrap(),
        "From source 1"
    );
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("target/output2.txt")).unwrap(),
        "From source 2"
    );
}
}