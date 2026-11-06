//! The context properties a graft declares and the values supplied for them.
//!
//! Validates supplied values against the declarations, coerces them to the declared type and
//! applies defaults.

use crate::error::GraftError;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Type a context property declares, written in lowercase in `.graft.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ContextDataType {
    /// A JSON array; nothing else is coerced into one.
    Array,
    /// A boolean; also accepts an integer or a text form such as `yes` or `0`.
    Boolean,
    /// A number; also accepts a string that parses as an integer or a float.
    Number,
    /// A string; any other value is converted to its text form.
    String,
}

/// One property a `.graft.yaml` declares; with no default value, the property is required.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ContextPropertyDefinition {
    /// Type a supplied value is coerced to.
    pub data_type: ContextDataType,

    /// Default value (if present, property is optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,

    /// Required text, shown in the error when a required value is missing.
    pub description: String,

    /// Name the value is supplied and looked up under; must not be empty.
    pub name: String,
}

/// Context values keyed by property name.
pub type ContextValues = HashMap<String, Value>;

/// A graft's context after validation: its declarations and the values resolved against them.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ValidatedContext {
    /// Property definitions from .graft.yaml.
    pub definitions: Vec<ContextPropertyDefinition>,

    /// Resolved values (after merging, defaults, and validation).
    pub values: ContextValues,
}

impl ValidatedContext {
    /// Returns a property's resolved value, or `None` if it was unset or never declared.
    #[must_use]
    #[inline]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// Returns a property's value in the text form a replacement writes.
    ///
    /// # Errors
    ///
    /// Returns an error if the property has no value or the value cannot be serialized.
    #[inline]
    pub fn get_as_string(&self, name: &str) -> Result<String> {
        let value = self.get(name).ok_or_else(|| {
            GraftError::configuration(format!("Context property not found: {name}"))
        })?;

        value_to_string(value)
    }

    /// Resolves supplied values against the definitions; a value no definition names is dropped.
    ///
    /// # Errors
    ///
    /// Returns an error listing every missing required property, or else every mistyped value.
    #[inline]
    pub fn new(
        definitions: Vec<ContextPropertyDefinition>,
        provided_values: ContextValues,
    ) -> Result<Self> {
        let values = validate_and_merge_values(&definitions, provided_values)?;

        Ok(Self {
            definitions,
            values,
        })
    }
}

/// Resolves each definition to its coerced value or its default; an empty string leaves it unset.
fn validate_and_merge_values(
    definitions: &[ContextPropertyDefinition],
    mut provided_values: ContextValues,
) -> Result<ContextValues> {
    let mut result = HashMap::new();
    let mut missing_required = Vec::new();
    let mut type_errors = Vec::new();

    for def in definitions {
        if let Some(value) = provided_values.remove(&def.name) {
            if is_empty_string(&value) {
                continue;
            }

            match validate_and_coerce_type(&def.name, &value, &def.data_type) {
                Ok(coerced_value) => {
                    result.insert(def.name.clone(), coerced_value);
                }
                Err(err) => {
                    type_errors.push(format!("  - {}: {}", def.name, err));
                }
            }
        } else if let Some(default_value) = def.default_value.as_ref() {
            result.insert(def.name.clone(), default_value.clone());
        } else {
            missing_required.push(format!(
                "  - {} ({}): {}",
                def.name,
                format!("{:?}", def.data_type).to_lowercase(),
                def.description
            ));
        }
    }

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

/// Matches the value that unsets a property: a JSON string of length zero.
const fn is_empty_string(value: &Value) -> bool {
    matches!(value, Value::String(str_val) if str_val.is_empty())
}

/// Applies the rule of `expected_type`; `name` only labels the error.
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

/// Never fails: a string is kept, a number or boolean becomes its text, the rest their JSON text.
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

/// Accepts a number, or a string that parses as an integer first and a float second.
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching on &Value; dereferencing would require ref bindings which conflict with clippy::ref_patterns"
)]
fn coerce_to_number(name: &str, value: &Value) -> Result<Value> {
    match value {
        Value::Number(_) => Ok(value.clone()),
        Value::String(str_val) => {
            if let Ok(int_val) = str_val.parse::<i64>() {
                return Ok(Value::Number(int_val.into()));
            }
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

/// Takes a boolean, an integer (non-zero is true) or any case of `true`/`yes`/`1`/`false`/`no`/`0`.
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

/// Passes an array through unchanged; nothing else is coerced into one.
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

/// Renders a value as replacement text: strings bare, arrays and objects as JSON, null as empty.
///
/// # Errors
///
/// Returns an error if an array or object cannot be serialized to JSON.
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
            serde_json::to_string(value).context("Failed to serialize context value to JSON")
        }
        Value::Null => Ok(String::new()),
    }
}

/// Overlays `child` on `parent`; an empty string in `child` removes the key.
#[must_use]
#[inline]
#[expect(
    clippy::iter_over_hash_type,
    reason = "iteration order is irrelevant for merging context values by key"
)]
pub fn merge_context_values(parent: ContextValues, child: ContextValues) -> ContextValues {
    let mut result = parent;
    for (key, value) in child {
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
