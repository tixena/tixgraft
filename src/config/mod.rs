//! Configuration management module
//!
//! Handles YAML configuration parsing, JSON schema validation, and configuration merging

pub mod context;
pub mod graft_yaml;
pub mod schema;
pub mod validation;
pub mod yaml;

use crate::cli::PullConfig;
use crate::system::System;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Global repository URL or account/repo format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    /// Global Git reference (branch, tag, or commit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    /// Global context values
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, Value>,

    /// List of pull operations
    pub pulls: Vec<PullConfig>,
}

impl Config {
    /// Load configuration from file
    pub fn load_from_file(system: &dyn System, path: &str) -> anyhow::Result<Self> {
        yaml::load_config(system, path)
    }

    /// Validate configuration against JSON schema
    pub fn validate(&self, system: &dyn System) -> anyhow::Result<()> {
        validation::validate_config(system, self)
    }
}
