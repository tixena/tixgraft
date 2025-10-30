//! Integration tests for to-command-line feature

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

    use assert_cmd::Command;
    use predicates::prelude::*;
    use std::io::Write as _;
    use tempfile::NamedTempFile;

    #[test]
    fn to_command_line_basic() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
repository: "my_organization/repo"
tag: "main"
pulls:
  - source: "src/path"
    target: "./dst/path"
    reset: true
"#
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--config")
            .arg(config_file.path())
            .arg("--to-command-line")
            .assert()
            .success()
            .stdout(predicate::str::contains("--repository"))
            .stdout(predicate::str::contains("my_organization/repo"))
            .stdout(predicate::str::contains("--tag"))
            .stdout(predicate::str::contains("main"))
            .stdout(predicate::str::contains("--pull-source"))
            .stdout(predicate::str::contains("src/path"))
            .stdout(predicate::str::contains("--pull-target"))
            .stdout(predicate::str::contains("./dst/path"))
            .stdout(predicate::str::contains("--pull-reset"));
    }

    #[test]
    fn to_command_line_json_format() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
repository: "my_organization/repo"
pulls:
  - source: "src"
    target: "dst"
"#
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        let output = cmd
            .arg("--config")
            .arg(config_file.path())
            .arg("--to-command-line")
            .arg("--output-format")
            .arg("json")
            .assert()
            .success();

        // Parse JSON output
        let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
        let json: Vec<String> = serde_json::from_str(&stdout).unwrap();

        assert!(json.contains(&"tixgraft".to_owned()));
        assert!(json.contains(&"--repository".to_owned()));
        assert!(json.contains(&"my_organization/repo".to_owned()));
        assert!(json.contains(&"--pull-source".to_owned()));
        assert!(json.contains(&"src".to_owned()));
    }

    #[test]
    fn to_command_line_with_replacements() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
repository: "my_organization/repo"
pulls:
  - source: "src"
    target: "dst"
    replacements:
      - source: "{{VAR1}}"
        target: "value1"
      - source: "{{VAR2}}"
        valueFromEnv: "MY_ENV"
"#
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        let output = cmd
            .env("MY_ENV", "test_value") // Pass env var to subprocess
            .arg("--config")
            .arg(config_file.path())
            .arg("--to-command-line")
            .output()
            .unwrap();

        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("--pull-replacement"));
        // The values might be quoted due to special characters
        assert!(stdout.contains("VAR1"));
        assert!(stdout.contains("value1"));
        assert!(stdout.contains("VAR2"));
        assert!(stdout.contains("env:MY_ENV"));
    }

    #[test]
    fn to_command_line_cli_overrides() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
repository: "my_organization/repo"
tag: "v1"
pulls:
  - source: "src"
    target: "dst"
"#
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--config")
            .arg(config_file.path())
            .arg("--to-command-line")
            .arg("--repository")
            .arg("override/repo")
            .arg("--tag")
            .arg("v2")
            .assert()
            .success()
            .stdout(predicate::str::contains("override/repo"))
            .stdout(predicate::str::contains("v2"))
            .stdout(predicate::str::contains("my_organization/repo").not());
    }

    #[test]
    fn to_command_line_with_commands() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
repository: "my_organization/repo"
pulls:
  - source: "src"
    target: "dst"
    commands:
      - "npm install"
      - "npm run build"
"#
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--config")
            .arg(config_file.path())
            .arg("--to-command-line")
            .assert()
            .success()
            .stdout(predicate::str::contains("--pull-commands"))
            .stdout(predicate::str::contains("npm install"))
            .stdout(predicate::str::contains("npm run build"));
    }

    #[test]
    fn to_command_line_multiple_pulls() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
repository: "my_organization/repo"
tag: "main"
pulls:
  - source: "path1"
    target: "target1"
    type: "file"
  - source: "path2"
    target: "target2"
    type: "directory"
    reset: true
"#
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        let output = cmd
            .arg("--config")
            .arg(config_file.path())
            .arg("--to-command-line")
            .output()
            .unwrap();

        let stdout = String::from_utf8(output.stdout).unwrap();

        // Verify both pulls are present
        assert!(stdout.contains("path1"));
        assert!(stdout.contains("target1"));
        assert!(stdout.contains("path2"));
        assert!(stdout.contains("target2"));
        assert!(stdout.contains("--pull-type"));
        assert!(stdout.contains("file"));
        assert!(stdout.contains("--pull-reset"));
    }

    #[test]
    fn to_command_line_per_pull_overrides() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
repository: "global/repo"
tag: "v1"
pulls:
  - source: "src1"
    target: "dst1"
  - source: "src2"
    target: "dst2"
    repository: "per-pull/repo"
    tag: "v2"
"#
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--config")
            .arg(config_file.path())
            .arg("--to-command-line")
            .assert()
            .success()
            .stdout(predicate::str::contains("--repository"))
            .stdout(predicate::str::contains("global/repo"))
            .stdout(predicate::str::contains("--pull-repository"))
            .stdout(predicate::str::contains("per-pull/repo"))
            .stdout(predicate::str::contains("--pull-tag"))
            .stdout(predicate::str::contains("v2"));
    }

    #[test]
    fn to_command_line_with_special_characters() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
repository: "my_organization/repo"
pulls:
  - source: "src with spaces"
    target: "dst with spaces"
    replacements:
      - source: "{{VAR}}"
        target: "value with $dollar"
"#
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        let output = cmd
            .arg("--config")
            .arg(config_file.path())
            .arg("--to-command-line")
            .output()
            .unwrap();

        let stdout = String::from_utf8(output.stdout).unwrap();

        // Verify special characters are properly escaped
        assert!(stdout.contains("\"src with spaces\""));
        assert!(stdout.contains("\"dst with spaces\""));
        assert!(stdout.contains("\\$")); // Dollar should be escaped
    }

    #[test]
    fn to_command_line_invalid_format() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
repository: "my_organization/repo"
pulls:
  - source: "src"
    target: "dst"
"#
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--config")
            .arg(config_file.path())
            .arg("--to-command-line")
            .arg("--output-format")
            .arg("invalid")
            .assert()
            .failure();
    }

    #[test]
    fn to_command_line_nonexistent_config() {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--config")
            .arg("nonexistent_file.yaml")
            .arg("--to-command-line")
            .assert()
            .failure()
            .code(1); // Configuration error
    }

    #[test]
    fn to_command_line_backslash_in_output() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
repository: "my_organization/repo"
pulls:
  - source: "src1"
    target: "dst1"
  - source: "src2"
    target: "dst2"
"#
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        let output = cmd
            .arg("--config")
            .arg(config_file.path())
            .arg("--to-command-line")
            .output()
            .unwrap();

        let stdout = String::from_utf8(output.stdout).unwrap();

        // Verify backslash continuations are present for multiline output
        assert!(stdout.contains(" \\\n"));
    }
}
