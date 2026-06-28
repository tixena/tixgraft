#![expect(
    clippy::unwrap_used,
    reason = "unwrap is acceptable in test code for brevity"
)]

use super::*;

#[test]
fn parse_simple_context() {
    let context = vec!["name=test".to_owned(), "port=8080".to_owned()];
    let json = vec![];
    let result = parse_context_args(&context, &json).unwrap();

    assert_eq!(result.get("name"), Some(&Value::String("test".to_owned())));
    assert_eq!(result.get("port"), Some(&Value::String("8080".to_owned())));
}

#[test]
fn parse_array_context() {
    let context = vec![
        "items=a".to_owned(),
        "items=b".to_owned(),
        "items=c".to_owned(),
    ];
    let json = vec![];
    let result = parse_context_args(&context, &json).unwrap();

    assert_eq!(
        result.get("items"),
        Some(&Value::Array(vec![
            Value::String("a".to_owned()),
            Value::String("b".to_owned()),
            Value::String("c".to_owned())
        ]))
    );
}

#[test]
fn parse_json_context() {
    let context = vec![];
    let json = vec![r#"config={"key":"value"}"#.to_owned()];
    let result = parse_context_args(&context, &json).unwrap();

    let expected = serde_json::json!({"key": "value"});
    assert_eq!(result.get("config"), Some(&expected));
}

#[test]
fn parse_mixed_context() {
    let context = vec!["name=test".to_owned()];
    let json = vec![r#"people=[{"name":"Alice"},{"name":"Bob"}]"#.to_owned()];
    let result = parse_context_args(&context, &json).unwrap();

    assert_eq!(result.get("name"), Some(&Value::String("test".to_owned())));
    assert_eq!(
        result.get("people"),
        Some(&serde_json::json!([{"name":"Alice"},{"name":"Bob"}]))
    );
}

#[test]
fn invalid_context_format() {
    let context = vec!["invalid".to_owned()];
    let json = vec![];
    let result = parse_context_args(&context, &json);

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Expected KEY=VALUE")
    );
}

#[test]
fn invalid_json() {
    let context = vec![];
    let json = vec!["config={invalid json}".to_owned()];
    let result = parse_context_args(&context, &json);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
}
