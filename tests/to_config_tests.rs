//! Integration tests for --to-config feature and roundtrip equivalence

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

    use assert_cmd::Command;
    use predicates::prelude::*;
    use std::io::Write as _;
    use std::process::Command as StdCommand;
    use tempfile::NamedTempFile;

    #[test]
    fn to_config_basic() {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--repository")
            .arg("my_organization/repo")
            .arg("--tag")
            .arg("main")
            .arg("--pull-source")
            .arg("src")
            .arg("--pull-target")
            .arg("dst")
            .arg("--to-config")
            .assert()
            .success()
            .stdout(predicate::str::contains("repository: my_organization/repo"))
            .stdout(predicate::str::contains("tag: main"))
            .stdout(predicate::str::contains("source: src"))
            .stdout(predicate::str::contains("target: dst"));
    }

    #[test]
    fn to_config_with_reset() {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--repository")
            .arg("my_organization/repo")
            .arg("--pull-source")
            .arg("src")
            .arg("--pull-target")
            .arg("dst")
            .arg("--pull-reset")
            .arg("true")
            .arg("--to-config")
            .assert()
            .success()
            .stdout(predicate::str::contains("reset: true"));
    }

    #[test]
    fn to_config_with_replacements() {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.env("MY_ENV", "test_value")
            .arg("--repository")
            .arg("my_organization/repo")
            .arg("--pull-source")
            .arg("src")
            .arg("--pull-target")
            .arg("dst")
            .arg("--pull-replacement")
            .arg("{{VAR}}=value")
            .arg("--to-config")
            .assert()
            .success()
            .stdout(predicate::str::contains("replacements:"))
            .stdout(predicate::str::contains("source:"))
            .stdout(predicate::str::contains("VAR"))
            .stdout(predicate::str::contains("target: value"));
    }

    #[test]
    fn to_config_with_env_replacement() {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.env("MY_ENV", "test_value")
            .arg("--repository")
            .arg("my_organization/repo")
            .arg("--pull-source")
            .arg("src")
            .arg("--pull-target")
            .arg("dst")
            .arg("--pull-replacement")
            .arg("{{VAR}}=env:MY_ENV")
            .arg("--to-config")
            .assert()
            .success()
            .stdout(predicate::str::contains("replacements:"))
            .stdout(predicate::str::contains("valueFromEnv: MY_ENV"));
    }

    #[test]
    fn to_config_from_existing_config_with_overrides() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            "
repository: original/repo
tag: v1
pulls:
  - source: src
    target: dst
"
        )
        .unwrap();

        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--config")
            .arg(config_file.path())
            .arg("--repository")
            .arg("override/repo")
            .arg("--to-config")
            .assert()
            .success()
            .stdout(predicate::str::contains("repository: override/repo"))
            .stdout(predicate::str::contains("original/repo").not());
    }

    #[test]
    fn to_config_conflicts_with_to_command_line() {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--repository")
            .arg("my_organization/repo")
            .arg("--pull-source")
            .arg("src")
            .arg("--pull-target")
            .arg("dst")
            .arg("--to-config")
            .arg("--to-command-line")
            .assert()
            .failure();
    }

    #[test]
    fn to_config_multiple_pulls() {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--repository")
            .arg("my_organization/repo")
            .arg("--pull-source")
            .arg("src1")
            .arg("--pull-target")
            .arg("dst1")
            .arg("--pull-source")
            .arg("src2")
            .arg("--pull-target")
            .arg("dst2")
            .arg("--pull-reset")
            .arg("true")
            .arg("--to-config")
            .assert()
            .success()
            .stdout(predicate::str::contains("src1"))
            .stdout(predicate::str::contains("dst1"))
            .stdout(predicate::str::contains("src2"))
            .stdout(predicate::str::contains("dst2"));
    }

    #[test]
    fn to_config_no_pulls_error() {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--repository")
            .arg("my_organization/repo")
            .arg("--to-config")
            .assert()
            .failure();
    }

    #[test]
    fn roundtrip_cli_to_config_to_cli() {
        // Step 1: Start with CLI arguments, convert to config
        let output1 = StdCommand::new("cargo")
            .args([
                "run",
                "--",
                "--repository",
                "my_organization/repo",
                "--tag",
                "v1.0.0",
                "--pull-source",
                "src",
                "--pull-target",
                "dst",
                "--pull-reset",
                "true",
                "--to-config",
            ])
            .output()
            .unwrap();

        assert!(output1.status.success());
        let config_yaml = String::from_utf8(output1.stdout).unwrap();

        // Step 2: Write config to temp file
        let mut temp_config = NamedTempFile::new().unwrap();
        temp_config.write_all(config_yaml.as_bytes()).unwrap();
        temp_config.flush().unwrap();

        // Step 3: Convert config back to CLI
        let output2 = StdCommand::new("cargo")
            .args([
                "run",
                "--",
                "--config",
                temp_config.path().to_str().unwrap(),
                "--to-command-line",
                "--output-format",
                "json",
            ])
            .output()
            .unwrap();

        assert!(output2.status.success());
        let cli_json = String::from_utf8(output2.stdout).unwrap();
        let cli_args: Vec<String> = serde_json::from_str(&cli_json).unwrap();

        // Step 4: Verify roundtrip preserves key values
        assert!(cli_args.contains(&"--repository".to_owned()));
        assert!(cli_args.contains(&"my_organization/repo".to_owned()));
        assert!(cli_args.contains(&"--tag".to_owned()));
        assert!(cli_args.contains(&"v1.0.0".to_owned()));
        assert!(cli_args.contains(&"--pull-source".to_owned()));
        assert!(cli_args.contains(&"src".to_owned()));
        assert!(cli_args.contains(&"--pull-target".to_owned()));
        assert!(cli_args.contains(&"dst".to_owned()));
        assert!(cli_args.contains(&"--pull-reset".to_owned()));
        assert!(cli_args.contains(&"true".to_owned()));
    }

    #[test]
    fn roundtrip_config_to_cli_to_config() {
        // Step 1: Start with a config file
        let original_config = "repository: my_organization/repo
tag: v1.0.0
pulls:
  - source: src
    target: dst
    reset: true
";

        let mut temp_config1 = NamedTempFile::new().unwrap();
        temp_config1.write_all(original_config.as_bytes()).unwrap();
        temp_config1.flush().unwrap();

        // Step 2: Convert to CLI
        let output1 = StdCommand::new("cargo")
            .args([
                "run",
                "--",
                "--config",
                temp_config1.path().to_str().unwrap(),
                "--to-command-line",
                "--output-format",
                "json",
            ])
            .output()
            .unwrap();

        assert!(output1.status.success());
        let cli_json = String::from_utf8(output1.stdout).unwrap();
        let cli_args: Vec<String> = serde_json::from_str(&cli_json).unwrap();

        // Step 3: Convert CLI back to config
        let mut args = vec!["run", "--"];
        args.extend(cli_args.iter().skip(1).map(String::as_str)); // Skip "tixgraft"
        args.push("--to-config");

        let output2 = StdCommand::new("cargo").args(&args).output().unwrap();

        assert!(output2.status.success());
        let roundtrip_config = String::from_utf8(output2.stdout).unwrap();

        // Step 4: Parse both configs and compare structures
        let original: serde_yaml::Value = serde_yaml::from_str(original_config).unwrap();

        // Filter out comments from roundtrip config
        let roundtrip_without_comments: String = roundtrip_config
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<&str>>()
            .join("\n");
        let roundtrip: serde_yaml::Value =
            serde_yaml::from_str(&roundtrip_without_comments).unwrap();

        // Compare key fields
        assert_eq!(original["repository"], roundtrip["repository"]);
        assert_eq!(original["tag"], roundtrip["tag"]);
        assert_eq!(
            original["pulls"][0]["source"],
            roundtrip["pulls"][0]["source"]
        );
        assert_eq!(
            original["pulls"][0]["target"],
            roundtrip["pulls"][0]["target"]
        );
        assert_eq!(
            original["pulls"][0]["reset"],
            roundtrip["pulls"][0]["reset"]
        );
    }

    #[test]
    fn roundtrip_with_replacements() {
        // Step 1: Start with config with replacements
        let original_config = r#"repository: my_organization/repo
pulls:
  - source: src
    target: dst
    replacements:
      - source: "{{VAR1}}"
        target: value1
      - source: "{{VAR2}}"
        valueFromEnv: MY_ENV
"#;

        let mut temp_config1 = NamedTempFile::new().unwrap();
        temp_config1.write_all(original_config.as_bytes()).unwrap();
        temp_config1.flush().unwrap();

        // Step 2: Convert to CLI
        let output1 = StdCommand::new("cargo")
            .env("MY_ENV", "test_value")
            .args([
                "run",
                "--",
                "--config",
                temp_config1.path().to_str().unwrap(),
                "--to-command-line",
                "--output-format",
                "json",
            ])
            .output()
            .unwrap();

        assert!(output1.status.success());
        let cli_json = String::from_utf8(output1.stdout).unwrap();
        let cli_args: Vec<String> = serde_json::from_str(&cli_json).unwrap();

        // Verify replacements are in CLI args
        assert!(cli_args.contains(&"--pull-replacement".to_owned()));

        // Step 3: Convert back to config
        let mut args = vec!["run", "--"];
        args.extend(cli_args.iter().skip(1).map(String::as_str));
        args.push("--to-config");

        let output2 = StdCommand::new("cargo")
            .env("MY_ENV", "test_value")
            .args(&args)
            .output()
            .unwrap();

        assert!(output2.status.success());
        let roundtrip_config = String::from_utf8(output2.stdout).unwrap();

        // Verify replacements are preserved
        assert!(roundtrip_config.contains("replacements:"));
        assert!(roundtrip_config.contains("VAR1"));
        assert!(roundtrip_config.contains("value1"));
        assert!(roundtrip_config.contains("VAR2"));
        assert!(roundtrip_config.contains("valueFromEnv: MY_ENV"));
    }

    #[test]
    fn to_config_with_commands() {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--repository")
            .arg("my_organization/repo")
            .arg("--pull-source")
            .arg("src")
            .arg("--pull-target")
            .arg("dst")
            .arg("--pull-commands")
            .arg("npm install,npm run build")
            .arg("--to-config")
            .assert()
            .success()
            .stdout(predicate::str::contains("commands:"));
    }

    #[test]
    fn to_config_header_comment() {
        let mut cmd = Command::cargo_bin("tixgraft").unwrap();
        cmd.arg("--repository")
            .arg("my_organization/repo")
            .arg("--pull-source")
            .arg("src")
            .arg("--pull-target")
            .arg("dst")
            .arg("--to-config")
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "# Generated by tixgraft --to-config",
            ));
    }
}
