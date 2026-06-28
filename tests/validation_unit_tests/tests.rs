#![expect(clippy::unwrap_used, reason = "This is a test module")]

use os_shim::mock::MockSystem;
use std::path::Path;
use tixgraft::cli::{PullConfig, ReplacementConfig};
use tixgraft::config::Config;
use tixgraft::config::validation::{
    validate_config, validate_config_with_base_dir, validate_path_safety, validate_repository_url,
};

#[test]
fn validate_repository_url_tst() {
    // Valid Git URLs
    validate_repository_url("my_organization/repo").unwrap();
    validate_repository_url("https://github.com/my_organization/repo.git").unwrap();
    validate_repository_url("git@github.com:my_organization/repo.git").unwrap();

    // Valid local paths (ONLY file: prefix)
    validate_repository_url("file:///path/to/repo").unwrap();
    validate_repository_url("file:/path/to/repo").unwrap();
    validate_repository_url("file:~/src/repo").unwrap();

    // Invalid URLs
    assert!(validate_repository_url("invalid-url").is_err());
    assert!(validate_repository_url("").is_err());

    // Paths without file: prefix should now be rejected
    assert!(validate_repository_url("~/src/repo").is_err());
    assert!(validate_repository_url("./local/repo").is_err());
    assert!(validate_repository_url("../local/repo").is_err());
    assert!(validate_repository_url("/absolute/path/to/repo").is_err());
}

#[test]
fn validate_path_safety_tst() {
    // Safe paths
    validate_path_safety("./some/path").unwrap();
    validate_path_safety("some/path").unwrap();
    validate_path_safety("./relative/path").unwrap();

    // Unsafe paths
    assert!(validate_path_safety("../../unsafe").is_err());
    assert!(validate_path_safety("/absolute/path").is_err());
}

fn make_config(pulls: Vec<PullConfig>, children: Vec<String>) -> Config {
    let yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping({
        let mut mapping = serde_yaml::Mapping::new();
        mapping.insert(
            serde_yaml::Value::String("repository".into()),
            serde_yaml::Value::String("org/repo".into()),
        );
        mapping.insert(
            serde_yaml::Value::String("tag".into()),
            serde_yaml::Value::String("main".into()),
        );
        mapping
    }))
    .unwrap();
    let mut config: Config = serde_yaml::from_str(&yaml).unwrap();
    config.pulls = pulls;
    config.children = children;
    config
}

fn make_pull(source: &str, target: &str) -> PullConfig {
    let yaml = format!("source: {source}\ntarget: {target}\ntype: directory");
    serde_yaml::from_str(&yaml).unwrap()
}

#[test]
fn validate_config_empty_pulls_and_children() {
    let system = MockSystem::new();
    let config = make_config(vec![], vec![]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("at least one"));
}

#[test]
fn validate_config_valid_with_pulls() {
    let system = MockSystem::new();
    let config = make_config(vec![make_pull("src", "./target")], vec![]);
    validate_config(&system, &config).unwrap();
}

#[test]
fn validate_config_valid_with_children() {
    let system = MockSystem::new()
        .with_file("/child/tixgraft.yaml", b"pulls: []")
        .unwrap();

    let config = make_config(vec![], vec!["child/tixgraft.yaml".to_owned()]);
    // Need base_dir since children paths resolve relative to it
    validate_config_with_base_dir(&system, &config, Some(Path::new("/"))).unwrap();
}

#[test]
fn validate_config_child_path_traversal() {
    let system = MockSystem::new();
    let config = make_config(vec![], vec!["../escape/tixgraft.yaml".to_owned()]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains(".."));
}

#[test]
fn validate_config_child_path_absolute() {
    let system = MockSystem::new();
    let config = make_config(vec![], vec!["/etc/tixgraft.yaml".to_owned()]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("absolute"));
}

#[test]
fn validate_config_child_path_empty() {
    let system = MockSystem::new();
    let config = make_config(vec![], vec!["  ".to_owned()]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[test]
fn validate_config_child_path_not_found() {
    let system = MockSystem::new();
    let config = make_config(vec![], vec!["missing/tixgraft.yaml".to_owned()]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn validate_pull_empty_source() {
    let system = MockSystem::new();
    let mut pull = make_pull("src", "./target");
    pull.source = "  ".to_owned();
    let config = make_config(vec![pull], vec![]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Source"));
}

#[test]
fn validate_pull_empty_target() {
    let system = MockSystem::new();
    let mut pull = make_pull("src", "./target");
    pull.target = "  ".to_owned();
    let config = make_config(vec![pull], vec![]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Target"));
}

#[test]
fn validate_pull_invalid_type() {
    let system = MockSystem::new();
    let mut pull = make_pull("src", "./target");
    pull.pull_type = "invalid".to_owned();
    let config = make_config(vec![pull], vec![]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid pull type")
    );
}

#[test]
fn validate_pull_unsafe_target() {
    let system = MockSystem::new();
    let pull = make_pull("src", "../../escape");
    let config = make_config(vec![pull], vec![]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("traversal"));
}

#[test]
fn validate_pull_empty_command() {
    let system = MockSystem::new();
    let mut pull = make_pull("src", "./target");
    pull.commands = vec!["  ".to_owned()];
    let config = make_config(vec![pull], vec![]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Command"));
}

#[test]
fn validate_replacement_empty_source() {
    let system = MockSystem::new();
    let mut pull = make_pull("src", "./target");
    pull.replacements = vec![ReplacementConfig::new(
        "  ".to_owned(),
        Some("val".to_owned()),
        None,
    )];
    let config = make_config(vec![pull], vec![]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("source cannot be empty")
    );
}

#[test]
fn validate_replacement_both_target_and_env() {
    let system = MockSystem::new();
    let mut pull = make_pull("src", "./target");
    pull.replacements = vec![ReplacementConfig::new(
        "{{X}}".to_owned(),
        Some("val".to_owned()),
        Some("ENV".to_owned()),
    )];
    let config = make_config(vec![pull], vec![]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Cannot specify both")
    );
}

#[test]
fn validate_replacement_neither_target_nor_env() {
    let system = MockSystem::new();
    let mut pull = make_pull("src", "./target");
    pull.replacements = vec![ReplacementConfig::new("{{X}}".to_owned(), None, None)];
    let config = make_config(vec![pull], vec![]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Must specify either")
    );
}

#[test]
fn validate_config_invalid_global_repo() {
    let system = MockSystem::new();
    let mut config = make_config(vec![make_pull("src", "./target")], vec![]);
    config.repository = Some("invalid-url".to_owned());
    let result = validate_config(&system, &config);
    assert!(result.is_err());
}

#[test]
fn validate_config_invalid_pull_repo() {
    let system = MockSystem::new();
    let mut pull = make_pull("src", "./target");
    pull.repository = Some("bad-url".to_owned());
    let config = make_config(vec![pull], vec![]);
    let result = validate_config(&system, &config);
    assert!(result.is_err());
}
