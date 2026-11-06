//! Tests that run the binary against repositories given as `file:` paths.
//!
//! They cover file and directory pulls, replacements, commands, `reset`, dry runs and context.

#![expect(clippy::unwrap_used, reason = "This is a test module")]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[test]
fn local_file_source_with_file_prefix() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir_all(temp_dir.path().join("source")).unwrap();
    fs::create_dir_all(temp_dir.path().join("source/templates")).unwrap();
    fs::write(
        temp_dir.path().join("source/templates/file.txt"),
        "Hello from local source",
    )
    .unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Completed pull operations successfully",
        ));

    let output_file = temp_dir.path().join("target/output.txt");
    assert!(output_file.exists());
    let content = fs::read_to_string(output_file).unwrap();
    assert_eq!(content, "Hello from local source");
}

#[test]
fn local_directory_source_with_file_prefix() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir_all(temp_dir.path().join("source")).unwrap();
    fs::create_dir_all(temp_dir.path().join("source/templates")).unwrap();
    fs::write(temp_dir.path().join("source/templates/file1.txt"), "File 1").unwrap();
    fs::write(temp_dir.path().join("source/templates/file2.txt"), "File 2").unwrap();
    fs::create_dir_all(temp_dir.path().join("source/templates/subdir")).unwrap();
    fs::write(
        temp_dir.path().join("source/templates/subdir/file3.txt"),
        "File 3",
    )
    .unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

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

    fs::create_dir_all(temp_dir.path().join("source")).unwrap();
    fs::write(temp_dir.path().join("source/file.txt"), "Content").unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    let output_file = temp_dir.path().join("target/output.txt");
    assert!(output_file.exists());
    assert_eq!(fs::read_to_string(output_file).unwrap(), "Content");
}

#[test]
fn local_source_with_relative_path() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir_all(temp_dir.path().join("source")).unwrap();
    fs::write(temp_dir.path().join("source/file.txt"), "Relative content").unwrap();

    let config = r#"
repository: "file:./source"
pulls:
  - source: "file.txt"
    target: "./target/output.txt"
    type: "file"
"#;
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    let output_file = temp_dir.path().join("target/output.txt");
    assert!(output_file.exists());
    assert_eq!(fs::read_to_string(output_file).unwrap(), "Relative content");
}

#[test]
fn local_source_with_replacements() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir_all(temp_dir.path().join("source")).unwrap();
    fs::write(
        temp_dir.path().join("source/template.txt"),
        "Hello {{NAME}}, your project is {{PROJECT}}!",
    )
    .unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    let output_file = temp_dir.path().join("target/output.txt");
    assert!(output_file.exists());
    let content = fs::read_to_string(output_file).unwrap();
    assert_eq!(content, "Hello Alice, your project is TixGraft!");
}

#[test]
fn local_source_with_env_var_replacement() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir_all(temp_dir.path().join("source")).unwrap();
    fs::write(
        temp_dir.path().join("source/template.txt"),
        "App: {{APP_NAME}}",
    )
    .unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .env("TEST_APP_NAME", "MyApp")
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    let output_file = temp_dir.path().join("target/output.txt");
    assert!(output_file.exists());
    let content = fs::read_to_string(output_file).unwrap();
    assert_eq!(content, "App: MyApp");
}

/// A file pull's commands run in the directory that holds the target file.
#[test]
fn local_source_with_commands() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir_all(temp_dir.path().join("source")).unwrap();
    fs::write(temp_dir.path().join("source/file.txt"), "Original").unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    assert!(temp_dir.path().join("target/file.txt").exists());

    let command_output = temp_dir.path().join("target/command_output.txt");
    assert!(command_output.exists());
    let content = fs::read_to_string(command_output).unwrap();
    assert!(content.contains("Command executed"));
}

#[test]
fn local_source_with_reset() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir_all(temp_dir.path().join("source")).unwrap();
    fs::write(temp_dir.path().join("source/new.txt"), "New content").unwrap();

    fs::create_dir_all(temp_dir.path().join("target")).unwrap();
    fs::write(temp_dir.path().join("target/old.txt"), "Old content").unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    assert!(temp_dir.path().join("target/new.txt").exists());
    assert!(!temp_dir.path().join("target/old.txt").exists());
}

#[test]
fn local_source_nonexistent_path() {
    let temp_dir = TempDir::new().unwrap();

    let config = r#"
repository: "file:///nonexistent/path/to/repo"
pulls:
  - source: "file.txt"
    target: "./output.txt"
    type: "file"
"#;
    fs::write(temp_dir.path().join("tixgraft.yaml"), config).unwrap();

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .failure();
}

#[test]
fn local_source_missing_file() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir_all(temp_dir.path().join("source")).unwrap();

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

    fs::create_dir_all(temp_dir.path().join("source1")).unwrap();
    fs::write(temp_dir.path().join("source1/file1.txt"), "From source 1").unwrap();

    fs::create_dir_all(temp_dir.path().join("source2")).unwrap();
    fs::write(temp_dir.path().join("source2/file2.txt"), "From source 2").unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

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

    fs::create_dir_all(temp_dir.path().join("source")).unwrap();
    fs::write(temp_dir.path().join("source/file.txt"), "Content").unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run preview"));

    assert!(!temp_dir.path().join("target").exists());
}

#[test]
fn local_source_per_pull_override() {
    let temp_dir = TempDir::new().unwrap();

    fs::create_dir_all(temp_dir.path().join("source1")).unwrap();
    fs::write(temp_dir.path().join("source1/file.txt"), "From source 1").unwrap();

    fs::create_dir_all(temp_dir.path().join("source2")).unwrap();
    fs::write(temp_dir.path().join("source2/file.txt"), "From source 2").unwrap();

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

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--config")
        .arg("tixgraft.yaml")
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(temp_dir.path().join("target/output1.txt")).unwrap(),
        "From source 1"
    );
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("target/output2.txt")).unwrap(),
        "From source 2"
    );
}

fn write_extends_template(root: &Path) -> PathBuf {
    let tmpl = root.join("tmpl/product_manager");
    fs::create_dir_all(&tmpl).unwrap();
    fs::write(
        tmpl.join(".graft.yaml"),
        r#"context:
  - name: extends
    description: "TOML array of parent templates"
    dataType: string
    defaultValue: '["document_reviewer"]'
replacements:
  - source: "{{EXTENDS}}"
    valueFromContext: extends
"#,
    )
    .unwrap();
    fs::write(tmpl.join("vigil.toml"), "extends = {{EXTENDS}}\n").unwrap();
    root.join("tmpl").canonicalize().unwrap()
}

#[test]
fn local_graft_yaml_value_from_context_cli() {
    let temp_dir = TempDir::new().unwrap();
    let tmpl_root = write_extends_template(temp_dir.path());

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--repository")
        .arg(format!("file:{}", tmpl_root.display()))
        .arg("--pull-source")
        .arg("product_manager")
        .arg("--pull-target")
        .arg("./pm")
        .arg("--pull-type")
        .arg("directory")
        .arg("--context")
        .arg(r#"extends=["FOOBAR"]"#)
        .assert()
        .success();

    let content = fs::read_to_string(temp_dir.path().join("pm/vigil.toml")).unwrap();
    assert_eq!(content.trim(), r#"extends = ["FOOBAR"]"#);
    assert!(!temp_dir.path().join("pm/.graft.yaml").exists());
}

#[test]
fn local_graft_yaml_value_from_context_default() {
    let temp_dir = TempDir::new().unwrap();
    let tmpl_root = write_extends_template(temp_dir.path());

    let mut cmd = Command::cargo_bin("tixgraft").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("--repository")
        .arg(format!("file:{}", tmpl_root.display()))
        .arg("--pull-source")
        .arg("product_manager")
        .arg("--pull-target")
        .arg("./pm")
        .arg("--pull-type")
        .arg("directory")
        .assert()
        .success();

    let content = fs::read_to_string(temp_dir.path().join("pm/vigil.toml")).unwrap();
    assert_eq!(content.trim(), r#"extends = ["document_reviewer"]"#);
}

fn write_required_context_template(root: &Path) -> PathBuf {
    let tmpl = root.join("tmpl/component");
    fs::create_dir_all(&tmpl).unwrap();
    fs::write(
        tmpl.join(".graft.yaml"),
        r#"context:
  - name: foo
    description: "required value"
    dataType: string
replacements:
  - source: "{{FOO}}"
    valueFromContext: foo
"#,
    )
    .unwrap();
    fs::write(tmpl.join("file.md"), "top={{FOO}}\n").unwrap();
    root.join("tmpl").canonicalize().unwrap()
}

#[test]
fn reset_keeps_the_target_when_required_context_is_missing() {
    let temp_dir = TempDir::new().unwrap();
    let tmpl_root = write_required_context_template(temp_dir.path());

    let pull = |context: Option<&str>| {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.current_dir(temp_dir.path())
            .arg("--repository")
            .arg(format!("file:{}", tmpl_root.display()))
            .arg("--pull-source")
            .arg("component")
            .arg("--pull-target")
            .arg("./out")
            .arg("--pull-type")
            .arg("directory")
            .arg("--pull-reset")
            .arg("true");
        if let Some(values) = context {
            cmd.arg("--context").arg(values);
        }
        cmd
    };

    pull(Some("foo=good")).assert().success();

    let good = temp_dir.path().join("out/file.md");
    assert_eq!(fs::read_to_string(&good).unwrap().trim(), "top=good");

    pull(None).assert().failure();

    assert_eq!(fs::read_to_string(&good).unwrap().trim(), "top=good");
    assert!(!temp_dir.path().join("out/.graft.yaml").exists());
}
