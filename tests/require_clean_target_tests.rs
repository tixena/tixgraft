//! Integration tests for the requireCleanTarget feature.

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use tixgraft::cli::PullConfig;
    use tixgraft::system::System as _;
    use tixgraft::system::real::RealSystem;

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
}
