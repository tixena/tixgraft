#![expect(clippy::unwrap_used, reason = "This is a test module")]

use tixgraft::config::schema::validate_against_schema;

/// Helper: parse a YAML string into a `serde_json::Value` for schema validation.
fn yaml_to_json(yaml: &str) -> serde_json::Value {
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
    serde_json::to_value(yaml_value).unwrap()
}

// ── Root-level context ──────────────────────────────────────────────

#[test]
fn root_context_with_string_values() {
    let config = yaml_to_json(
        r#"
repository: "tixena/scaffold"
tag: "master"
context:
  extends: '["cautious"]'
  appName: "my-app"
pulls:
  - source: "templates/service"
    target: "./service"
"#,
    );
    validate_against_schema(&config).unwrap();
}

#[test]
fn root_context_with_mixed_value_types() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
tag: "main"
context:
  name: "my-project"
  port: 8080
  debug: true
  tags:
    - "api"
    - "v2"
pulls:
  - source: "src"
    target: "./dest"
"#,
    );
    validate_against_schema(&config).unwrap();
}

#[test]
fn root_context_empty_object() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
context: {}
pulls:
  - source: "src"
    target: "./dest"
"#,
    );
    validate_against_schema(&config).unwrap();
}

// ── Pull-level context ──────────────────────────────────────────────

#[test]
fn pull_context_with_string_values() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
pulls:
  - source: "templates/api"
    target: "./api"
    context:
      serviceName: "my-api"
      region: "us-east-1"
"#,
    );
    validate_against_schema(&config).unwrap();
}

#[test]
fn pull_context_with_mixed_value_types() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
pulls:
  - source: "templates/api"
    target: "./api"
    context:
      name: "service"
      replicas: 3
      enabled: true
      features:
        - "auth"
        - "logging"
"#,
    );
    validate_against_schema(&config).unwrap();
}

#[test]
fn pull_context_empty_object() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
pulls:
  - source: "src"
    target: "./dest"
    context: {}
"#,
    );
    validate_against_schema(&config).unwrap();
}

// ── Both root and pull context ──────────────────────────────────────

#[test]
fn root_and_pull_context_together() {
    let config = yaml_to_json(
        r#"
repository: "tixena/scaffold"
tag: "master"
context:
  extends: '["cautious"]'
  globalVar: "shared"
pulls:
  - source: "vigil/templates/tech_task_writer"
    target: "./tech_task_writer"
    reset: true
    context:
      localVar: "per-pull-only"
"#,
    );
    validate_against_schema(&config).unwrap();
}

#[test]
fn multiple_pulls_with_different_contexts() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
context:
  shared: "global-value"
pulls:
  - source: "templates/api"
    target: "./api"
    context:
      serviceName: "api-service"
  - source: "templates/worker"
    target: "./worker"
    context:
      serviceName: "worker-service"
      concurrency: 4
"#,
    );
    validate_against_schema(&config).unwrap();
}

// ── Configs without context still pass ───────────────────────────────

#[test]
fn config_without_context_still_valid() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
tag: "main"
pulls:
  - source: "src"
    target: "./dest"
    type: "directory"
    reset: true
    commands:
      - "npm install"
    replacements:
      - source: "{{NAME}}"
        target: "my-project"
"#,
    );
    validate_against_schema(&config).unwrap();
}

// ── Invalid property still rejected ─────────────────────────────────

#[test]
fn unknown_root_property_rejected() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
unknownField: "should fail"
pulls:
  - source: "src"
    target: "./dest"
"#,
    );
    let result = validate_against_schema(&config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("unknownField"));
}

#[test]
fn unknown_pull_property_rejected() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
pulls:
  - source: "src"
    target: "./dest"
    badProperty: true
"#,
    );
    let result = validate_against_schema(&config);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("badProperty"));
}

// ── Context with nested objects ─────────────────────────────────────

#[test]
fn root_context_with_nested_objects() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
context:
  database:
    host: "localhost"
    port: 5432
  features:
    auth: true
    logging: false
pulls:
  - source: "src"
    target: "./dest"
"#,
    );
    validate_against_schema(&config).unwrap();
}

#[test]
fn pull_context_with_nested_objects() {
    let config = yaml_to_json(
        r#"
repository: "org/repo"
pulls:
  - source: "src"
    target: "./dest"
    context:
      config:
        nested:
          deep: "value"
"#,
    );
    validate_against_schema(&config).unwrap();
}
