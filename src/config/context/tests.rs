#![expect(
    clippy::unwrap_used,
    reason = "unwrap is acceptable in tests for concise assertions"
)]
#![expect(
    clippy::shadow_unrelated,
    reason = "test functions reuse variable names for clarity"
)]

use super::*;
use serde_json::json;

#[test]
fn validate_required_properties() {
    let definitions = vec![
        ContextPropertyDefinition {
            data_type: ContextDataType::String,
            default_value: None,
            description: "Project name".to_owned(),
            name: "projectName".to_owned(),
        },
        ContextPropertyDefinition {
            data_type: ContextDataType::Number,
            default_value: Some(json!(10_i64)),
            description: "Max GB per pod".to_owned(),
            name: "maxGbPerPod".to_owned(),
        },
    ];

    // Missing required property
    let values = HashMap::new();
    let result = validate_and_merge_values(&definitions, values);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Missing required context properties")
    );

    // Provide required property
    let mut values = HashMap::new();
    values.insert("projectName".to_owned(), json!("my-app"));
    let result = validate_and_merge_values(&definitions, values);
    assert!(result.is_ok());
    let merged = result.unwrap();
    assert_eq!(merged.get("projectName"), Some(&json!("my-app")));
    assert_eq!(merged.get("maxGbPerPod"), Some(&json!(10_i64))); // default applied
}

#[test]
fn type_coercion_string_to_number() {
    let definitions = vec![ContextPropertyDefinition {
        data_type: ContextDataType::Number,
        default_value: None,
        description: "Port number".to_owned(),
        name: "port".to_owned(),
    }];

    // String "8080" should coerce to number
    let mut values = HashMap::new();
    values.insert("port".to_owned(), json!("8080"));
    let result = validate_and_merge_values(&definitions, values);
    assert!(result.is_ok());
    let merged = result.unwrap();
    assert_eq!(merged.get("port"), Some(&json!(8080_i64)));
}

#[test]
fn type_coercion_string_to_boolean() {
    let definitions = vec![ContextPropertyDefinition {
        data_type: ContextDataType::Boolean,
        default_value: None,
        description: "Enable feature".to_owned(),
        name: "enabled".to_owned(),
    }];

    // String "true" should coerce to boolean
    let mut values = HashMap::new();
    values.insert("enabled".to_owned(), json!("true"));
    let result = validate_and_merge_values(&definitions, values);
    assert!(result.is_ok());
    let merged = result.unwrap();
    assert_eq!(merged.get("enabled"), Some(&json!(true)));
}

#[test]
fn empty_string_removes_property() {
    let definitions = vec![ContextPropertyDefinition {
        data_type: ContextDataType::String,
        default_value: Some(json!("default")),
        description: "Optional property".to_owned(),
        name: "optional".to_owned(),
    }];

    // Empty string should remove property
    let mut values = HashMap::new();
    values.insert("optional".to_owned(), json!(""));
    let result = validate_and_merge_values(&definitions, values);
    assert!(result.is_ok());
    let merged = result.unwrap();
    assert_eq!(merged.get("optional"), None);
}

#[test]
fn array_validation() {
    let definitions = vec![ContextPropertyDefinition {
        data_type: ContextDataType::Array,
        default_value: None,
        description: "List of items".to_owned(),
        name: "items".to_owned(),
    }];

    // Valid array
    let mut values = HashMap::new();
    values.insert("items".to_owned(), json!(["a", "b", "c"]));
    let result = validate_and_merge_values(&definitions, values);
    result.unwrap();

    // Invalid (not an array)
    let mut values_2 = HashMap::new();
    values_2.insert("items".to_owned(), json!("not-an-array"));
    let result_2 = validate_and_merge_values(&definitions, values_2);
    result_2.unwrap_err();
}

#[test]
fn coerce_number_to_string() {
    // Number should coerce to string
    let val = coerce_to_string(&json!(42_i64));
    assert_eq!(val, json!("42"));
}

#[test]
fn coerce_bool_to_string() {
    let val = coerce_to_string(&json!(true));
    assert_eq!(val, json!("true"));
}

#[test]
fn coerce_null_to_string() {
    let val = coerce_to_string(&json!(null));
    assert_eq!(val, json!("null"));
}

#[test]
fn coerce_string_float_to_number() {
    coerce_to_number("test", &json!("3.5")).unwrap();
}

#[test]
fn coerce_invalid_string_to_number() {
    coerce_to_number("test", &json!("not-a-number")).unwrap_err();
}

#[test]
fn coerce_bool_to_number_fails() {
    coerce_to_number("test", &json!(true)).unwrap_err();
}

#[test]
fn coerce_number_to_boolean() {
    // Non-zero integer to boolean
    let result = coerce_to_boolean("test", &json!(1_i64));
    assert_eq!(result.unwrap(), json!(true));

    let result = coerce_to_boolean("test", &json!(0_i64));
    assert_eq!(result.unwrap(), json!(false));
}

#[test]
fn coerce_string_yes_no_to_boolean() {
    assert_eq!(
        coerce_to_boolean("test", &json!("yes")).unwrap(),
        json!(true)
    );
    assert_eq!(
        coerce_to_boolean("test", &json!("no")).unwrap(),
        json!(false)
    );
    assert_eq!(coerce_to_boolean("test", &json!("1")).unwrap(), json!(true));
    assert_eq!(
        coerce_to_boolean("test", &json!("0")).unwrap(),
        json!(false)
    );
    assert_eq!(
        coerce_to_boolean("test", &json!("false")).unwrap(),
        json!(false)
    );
}

#[test]
fn coerce_invalid_string_to_boolean() {
    coerce_to_boolean("test", &json!("maybe")).unwrap_err();
}

#[test]
fn coerce_null_to_boolean_fails() {
    coerce_to_boolean("test", &json!(null)).unwrap_err();
}

#[test]
fn coerce_float_to_boolean_fails() {
    // Float numbers can't coerce to boolean (no as_i64)
    coerce_to_boolean("test", &json!(2.5_f64)).unwrap_err();
}

#[test]
fn type_error_in_validation() {
    let definitions = vec![ContextPropertyDefinition {
        data_type: ContextDataType::Number,
        default_value: None,
        description: "Count".to_owned(),
        name: "count".to_owned(),
    }];

    let mut values = HashMap::new();
    values.insert("count".to_owned(), json!(true)); // bool can't be number
    let result = validate_and_merge_values(&definitions, values);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid context values")
    );
}
