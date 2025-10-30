//! Context management for grafts
//!
//! Provides data structures and validation for context properties that can be used
//! in text replacements and other graft operations.

use crate::error::GraftError;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Data type for context properties
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ContextDataType {
    String,
    Number,
    Boolean,
    Array,
}

/// Definition of a context property in .graft.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ContextPropertyDefinition {
    /// Property name
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// Data type of the property
    pub data_type: ContextDataType,

    /// Default value (if present, property is optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,
}

/// Context values provided by user (property name -> value)
pub type ContextValues = HashMap<String, Value>;

/// Validated context with definitions and values
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ValidatedContext {
    /// Property definitions from .graft.yaml
    pub definitions: Vec<ContextPropertyDefinition>,

    /// Resolved values (after merging, defaults, and validation)
    pub values: ContextValues,
}

impl ValidatedContext {
    /// Create a new validated context
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The context values are invalid
    #[inline]
    pub fn new(
        definitions: Vec<ContextPropertyDefinition>,
        provided_values: ContextValues,
    ) -> Result<Self> {
        // Validate and merge values
        let values = validate_and_merge_values(&definitions, provided_values)?;

        Ok(Self {
            definitions,
            values,
        })
    }

    /// Get a context value by name
    #[must_use]
    #[inline]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// Get a context value as a string
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The context property is not found
    #[inline]
    pub fn get_as_string(&self, name: &str) -> Result<String> {
        let value = self.get(name).ok_or_else(|| {
            GraftError::configuration(format!("Context property not found: {name}"))
        })?;

        value_to_string(value)
    }
}

/// Validate context values against definitions and apply defaults
fn validate_and_merge_values(
    definitions: &[ContextPropertyDefinition],
    mut provided_values: ContextValues,
) -> Result<ContextValues> {
    let mut result = HashMap::new();
    let mut missing_required = Vec::new();
    let mut type_errors = Vec::new();

    for def in definitions {
        // Check if value is provided
        if let Some(value) = provided_values.remove(&def.name) {
            // Empty string means remove from context
            if is_empty_string(&value) {
                continue;
            }

            // Validate and coerce type
            match validate_and_coerce_type(&def.name, &value, &def.data_type) {
                Ok(coerced_value) => {
                    result.insert(def.name.clone(), coerced_value);
                }
                Err(e) => {
                    type_errors.push(format!("  - {}: {}", def.name, e));
                }
            }
        } else if let Some(default_value) = def.default_value.as_ref() {
            // Use default value
            result.insert(def.name.clone(), default_value.clone());
        } else {
            // Required property is missing
            missing_required.push(format!(
                "  - {} ({}): {}",
                def.name,
                format!("{:?}", def.data_type).to_lowercase(),
                def.description
            ));
        }
    }

    // Report validation errors
    if !missing_required.is_empty() {
        return Err(GraftError::configuration(format!(
            "Missing required context properties:\n{}",
            missing_required.join("\n")
        ))
        .into());
    }

    if !type_errors.is_empty() {
        return Err(GraftError::configuration(format!(
            "Invalid context values:\n{}",
            type_errors.join("\n")
        ))
        .into());
    }

    Ok(result)
}

/// Check if a value is an empty string
const fn is_empty_string(value: &Value) -> bool {
    matches!(value, Value::String(s) if s.is_empty())
}

/// Validate and coerce a value to match the expected data type
fn validate_and_coerce_type(
    name: &str,
    value: &Value,
    expected_type: &ContextDataType,
) -> Result<Value> {
    match *expected_type {
        ContextDataType::String => Ok(coerce_to_string(value)),
        ContextDataType::Number => coerce_to_number(name, value),
        ContextDataType::Boolean => coerce_to_boolean(name, value),
        ContextDataType::Array => validate_array(name, value),
    }
}

/// Coerce a value to string
fn coerce_to_string(value: &Value) -> Value {
    match *value {
        Value::String(_) => value.clone(),
        Value::Number(ref n) => Value::String(n.to_string()),
        Value::Bool(b) => Value::String(b.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => Value::String(value.to_string()),
    }
}

/// Coerce a value to number
fn coerce_to_number(name: &str, value: &Value) -> Result<Value> {
    match *value {
        Value::Number(_) => Ok(value.clone()),
        Value::String(ref s) => {
            // Try to parse as integer first
            if let Ok(i) = s.parse::<i64>() {
                return Ok(Value::Number(i.into()));
            }
            // Try to parse as float
            if let Ok(f) = s.parse::<f64>() {
                return Ok(serde_json::Number::from_f64(f)
                    .map(Value::Number)
                    .ok_or_else(|| {
                        GraftError::configuration(format!("Invalid number value for '{name}': {s}"))
                    })?);
            }
            Err(GraftError::configuration(format!(
                "Cannot coerce '{s}' to number for property '{name}'"
            ))
            .into())
        }
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => {
            Err(GraftError::configuration(format!(
                "Cannot coerce {value:?} to number for property '{name}'"
            ))
            .into())
        }
    }
}

/// Coerce a value to boolean
fn coerce_to_boolean(name: &str, value: &Value) -> Result<Value> {
    match *value {
        Value::Bool(_) => Ok(value.clone()),
        Value::String(ref s) => match s.to_lowercase().as_str() {
            "true" | "yes" | "1" => Ok(Value::Bool(true)),
            "false" | "no" | "0" => Ok(Value::Bool(false)),
            _ => Err(GraftError::configuration(format!(
                "Cannot coerce '{s}' to boolean for property '{name}' (expected true/false/yes/no/1/0)"
            ))
            .into()),
        },
        Value::Number(ref n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Bool(i != 0))
            } else {
                Err(GraftError::configuration(format!(
                    "Cannot coerce number to boolean for property '{name}'"
                ))
                .into())
            }
        }
        Value::Null | Value::Array(_) | Value::Object(_) => Err(GraftError::configuration(format!(
            "Cannot coerce {value:?} to boolean for property '{name}'"
        ))
        .into()),
    }
}

/// Validate array type (no coercion, must be proper array)
fn validate_array(name: &str, value: &Value) -> Result<Value> {
    match *value {
        Value::Array(_) => Ok(value.clone()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_) => {
            Err(GraftError::configuration(format!(
                "Property '{name}' must be an array, got: {value:?}"
            ))
            .into())
        }
    }
}

/// Convert a JSON value to a string for replacement
///
/// # Errors
///
/// Returns an error if:
/// - The value cannot be serialized to a string
#[inline]
pub fn value_to_string(value: &Value) -> Result<String> {
    match *value {
        Value::String(ref s) => Ok(s.clone()),
        Value::Number(ref n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Array(_) | Value::Object(_) => {
            // For complex types, serialize as JSON
            serde_json::to_string(value).context("Failed to serialize context value to JSON")
        }
        Value::Null => Ok(String::new()),
    }
}

/// Merge two context value maps (child overrides parent)
///
/// # Returns
///
/// Returns the merged context values
#[must_use]
#[inline]
pub fn merge_context_values(parent: ContextValues, child: ContextValues) -> ContextValues {
    let mut result = parent;
    for (key, value) in child {
        // Empty string means remove from context
        if is_empty_string(&value) {
            result.remove(&key);
        } else {
            result.insert(key, value);
        }
    }
    result
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
#[expect(clippy::shadow_unrelated)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validate_required_properties() {
        let definitions = vec![
            ContextPropertyDefinition {
                name: "projectName".to_owned(),
                description: "Project name".to_owned(),
                data_type: ContextDataType::String,
                default_value: None,
            },
            ContextPropertyDefinition {
                name: "maxGbPerPod".to_owned(),
                description: "Max GB per pod".to_owned(),
                data_type: ContextDataType::Number,
                default_value: Some(json!(10)),
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
        assert_eq!(merged.get("maxGbPerPod"), Some(&json!(10))); // default applied
    }

    #[test]
    fn type_coercion_string_to_number() {
        let definitions = vec![ContextPropertyDefinition {
            name: "port".to_owned(),
            description: "Port number".to_owned(),
            data_type: ContextDataType::Number,
            default_value: None,
        }];

        // String "8080" should coerce to number
        let mut values = HashMap::new();
        values.insert("port".to_owned(), json!("8080"));
        let result = validate_and_merge_values(&definitions, values);
        assert!(result.is_ok());
        let merged = result.unwrap();
        assert_eq!(merged.get("port"), Some(&json!(8080)));
    }

    #[test]
    fn type_coercion_string_to_boolean() {
        let definitions = vec![ContextPropertyDefinition {
            name: "enabled".to_owned(),
            description: "Enable feature".to_owned(),
            data_type: ContextDataType::Boolean,
            default_value: None,
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
            name: "optional".to_owned(),
            description: "Optional property".to_owned(),
            data_type: ContextDataType::String,
            default_value: Some(json!("default")),
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
            name: "items".to_owned(),
            description: "List of items".to_owned(),
            data_type: ContextDataType::Array,
            default_value: None,
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
    fn merge_context_values_tst() {
        let mut parent = HashMap::new();
        parent.insert("a".to_owned(), json!("parent-a"));
        parent.insert("b".to_owned(), json!("parent-b"));

        let mut child = HashMap::new();
        child.insert("b".to_owned(), json!("child-b"));
        child.insert("c".to_owned(), json!("child-c"));

        let merged = merge_context_values(parent, child);
        assert_eq!(merged.get("a"), Some(&json!("parent-a")));
        assert_eq!(merged.get("b"), Some(&json!("child-b")));
        assert_eq!(merged.get("c"), Some(&json!("child-c")));
    }

    #[test]
    fn value_to_string_tst() {
        assert_eq!(value_to_string(&json!("hello")).unwrap(), "hello");
        assert_eq!(value_to_string(&json!(42)).unwrap(), "42");
        assert_eq!(value_to_string(&json!(true)).unwrap(), "true");
        assert_eq!(
            value_to_string(&json!(["a", "b"])).unwrap(),
            "[\"a\",\"b\"]"
        );
    }
}
