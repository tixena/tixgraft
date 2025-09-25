//! Configuration management module
//! 
//! Handles YAML configuration parsing, JSON schema validation, and configuration merging

pub mod schema;
pub mod yaml;
pub mod validation;

use serde::{Deserialize, Serialize};
use crate::cli::PullConfig;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Global repository URL or account/repo format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    
    /// Global Git reference (branch, tag, or commit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    
    /// List of pull operations
    pub pulls: Vec<PullConfig>,
}

impl Config {
    /// Load configuration from file
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        yaml::load_config(path)
    }
    
    /// Validate configuration against JSON schema
    pub fn validate(&self) -> anyhow::Result<()> {
        validation::validate_config(self)
    }
}
