//! Context management for grafts.
//!
//! Provides data structures and validation for context properties that can be used
//! in text replacements and other graft operations.

use crate::error::GraftError;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Data type for context properties.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ContextDataType {
    Array,
    Boolean,
    Number,
    String,
}

/// Definition of a context property in .graft.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ContextPropertyDefinition {
    /// Data type of the property.
    pub data_type: ContextDataType,

    /// Default value (if present, property is optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,

    /// Human-readable description.
    pub description: String,

    /// Property name.
    pub name: String,
}

/// Context values provided by user (property name -> value).
pub type ContextValues = HashMap<String, Value>;

/// Validated context with definitions and values.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ValidatedContext {
    /// Property definitions from .graft.yaml.
    pub definitions: Vec<ContextPropertyDefinition>,

    /// Resolved values (after merging, defaults, and validation).
    pub values: ContextValues,
}

impl ValidatedContext {
    /// Get a context value by name.
    #[must_use]
    #[inline]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// Get a context value as a string.
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

    /// Create a new validated context.
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
}

/// Validate context values against definitions and apply defaults.
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
                Err(err) => {
                    type_errors.push(format!("  - {}: {}", def.name, err));
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

/// Check if a value is an empty string.
const fn is_empty_string(value: &Value) -> bool {
    matches!(value, Value::String(str_val) if str_val.is_empty())
}

/// Validate and coerce a value to match the expected data type.
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

/// Coerce a value to string.
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching on &Value; dereferencing would require ref bindings which conflict with clippy::ref_patterns"
)]
fn coerce_to_string(value: &Value) -> Value {
    match value {
        Value::String(_) => value.clone(),
        Value::Number(num) => Value::String(num.to_string()),
        Value::Bool(flag) => Value::String(flag.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => Value::String(value.to_string()),
    }
}

/// Coerce a value to number.
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching on &Value; dereferencing would require ref bindings which conflict with clippy::ref_patterns"
)]
fn coerce_to_number(name: &str, value: &Value) -> Result<Value> {
    match value {
        Value::Number(_) => Ok(value.clone()),
        Value::String(str_val) => {
            // Try to parse as integer first
            if let Ok(int_val) = str_val.parse::<i64>() {
                return Ok(Value::Number(int_val.into()));
            }
            // Try to parse as float
            if let Ok(float_val) = str_val.parse::<f64>() {
                return Ok(serde_json::Number::from_f64(float_val)
                    .map(Value::Number)
                    .ok_or_else(|| {
                        GraftError::configuration(format!(
                            "Invalid number value for '{name}': {str_val}"
                        ))
                    })?);
            }
            Err(GraftError::configuration(format!(
                "Cannot coerce '{str_val}' to number for property '{name}'"
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

/// Coerce a value to boolean.
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching on &Value; dereferencing would require ref bindings which conflict with clippy::ref_patterns"
)]
fn coerce_to_boolean(name: &str, value: &Value) -> Result<Value> {
    match value {
        Value::Bool(_) => Ok(value.clone()),
        Value::String(str_val) => match str_val.to_lowercase().as_str() {
            "true" | "yes" | "1" => Ok(Value::Bool(true)),
            "false" | "no" | "0" => Ok(Value::Bool(false)),
            _ => Err(GraftError::configuration(format!(
                "Cannot coerce '{str_val}' to boolean for property '{name}' (expected true/false/yes/no/1/0)"
            ))
            .into()),
        },
        Value::Number(num) => num.as_i64().map_or_else(
            || {
                Err(GraftError::configuration(format!(
                    "Cannot coerce number to boolean for property '{name}'"
                ))
                .into())
            },
            |int_val| Ok(Value::Bool(int_val != 0_i64)),
        ),
        Value::Null | Value::Array(_) | Value::Object(_) => Err(GraftError::configuration(format!(
            "Cannot coerce {value:?} to boolean for property '{name}'"
        ))
        .into()),
    }
}

/// Validate array type (no coercion, must be proper array).
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching on &Value; dereferencing would require ref bindings which conflict with clippy::ref_patterns"
)]
fn validate_array(name: &str, value: &Value) -> Result<Value> {
    match value {
        Value::Array(_) => Ok(value.clone()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_) => {
            Err(GraftError::configuration(format!(
                "Property '{name}' must be an array, got: {value:?}"
            ))
            .into())
        }
    }
}

/// Convert a JSON value to a string for replacement.
///
/// # Errors
///
/// Returns an error if:
/// - The value cannot be serialized to a string
#[inline]
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching on &Value; dereferencing would require ref bindings which conflict with clippy::ref_patterns"
)]
pub fn value_to_string(value: &Value) -> Result<String> {
    match value {
        Value::String(str_val) => Ok(str_val.clone()),
        Value::Number(num) => Ok(num.to_string()),
        Value::Bool(flag) => Ok(flag.to_string()),
        Value::Array(_) | Value::Object(_) => {
            // For complex types, serialize as JSON
            serde_json::to_string(value).context("Failed to serialize context value to JSON")
        }
        Value::Null => Ok(String::new()),
    }
}

/// Merge two context value maps (child overrides parent).
///
/// # Returns
///
/// Returns the merged context values.
#[must_use]
#[inline]
#[expect(
    clippy::iter_over_hash_type,
    reason = "iteration order is irrelevant for merging context values by key"
)]
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
mod tests;
