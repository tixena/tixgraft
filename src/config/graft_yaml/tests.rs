#![expect(
    clippy::unwrap_used,
    reason = "unwrap is acceptable in test code for brevity"
)]

use super::*;

#[test]
fn context_property_validation() {
    let yaml = r#"
context:
  - name: ""
    description: "Empty name"
    dataType: string
"#;
    let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
    config.validate().unwrap_err();
}

#[test]
fn default_value_type_validation() {
    let yaml = r#"
context:
  - name: port
    description: Port number
    dataType: number
    defaultValue: "not-a-number"
"#;
    let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
    config.validate().unwrap_err();
}

#[test]
fn context_empty_description() {
    let yaml = "context:\n  - name: myProp\n    description: \"\"\n    dataType: string\n";
    let config: GraftConfig = serde_yaml::from_str(yaml).unwrap();
    config.validate().unwrap_err();
}

#[test]
fn validate_value_type_tst() {
    use serde_json::json;
    validate_value_type("x", &json!("hello"), &ContextDataType::String).unwrap();
    validate_value_type("x", &json!(42_i64), &ContextDataType::String).unwrap_err();
    validate_value_type("x", &json!(42_i64), &ContextDataType::Number).unwrap();
    validate_value_type("x", &json!(true), &ContextDataType::Boolean).unwrap();
    validate_value_type("x", &json!("hi"), &ContextDataType::Boolean).unwrap_err();
    validate_value_type("x", &json!([1_i32, 2_i32]), &ContextDataType::Array).unwrap();
    validate_value_type("x", &json!("str"), &ContextDataType::Array).unwrap_err();
}
