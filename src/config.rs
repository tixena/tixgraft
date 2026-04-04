//! Configuration management module.
//!
//! Handles YAML configuration parsing, JSON schema validation, and configuration merging.

pub mod context;
pub mod graft_yaml;
pub mod schema;
pub mod validation;
pub mod yaml;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::PullConfig;
use os_shim::System;

/// Main configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "field order matches YAML config schema for readability"
)]
pub struct Config {
    /// Global repository URL or account/repo format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    /// Global Git reference (branch, tag, or commit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    /// Global context values.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, Value>,

    /// List of pull operations.
    #[serde(default)]
    pub pulls: Vec<PullConfig>,

    /// Paths to child tixgraft.yaml files to execute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,

    /// If true, execute children before parent pulls (default: false).
    #[serde(
        default,
        skip_serializing_if = "std::ops::Not::not",
        rename = "processChildrenFirst"
    )]
    pub process_children_first: bool,
}

impl Config {
    /// Load configuration from file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration file cannot be loaded or parsed.
    #[inline]
    pub fn load_from_file(system: &dyn System, path: &str) -> anyhow::Result<Self> {
        yaml::load_config(system, path)
    }

    /// Validate configuration against JSON schema.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration is invalid.
    #[inline]
    pub fn validate(&self, system: &dyn System) -> anyhow::Result<()> {
        validation::validate_config(system, self)
    }
}
