//! The `tixgraft.yaml` configuration type and the modules that load and validate it.
//!
//! Also holds the `.graft.yaml` format and the context values a graft declares.

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

/// Root of a `tixgraft.yaml` file: shared defaults, the pulls and the child configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "field order matches YAML config schema for readability"
)]
pub struct Config {
    /// Repository for pulls that name none: a URL, `account/repo` or a local path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    /// Git reference (branch, tag, or commit) for pulls that name none; `main` when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    /// Context values shared by every pull; a pull's own values override them.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, Value>,

    /// Pulls to run, in order.
    #[serde(default)]
    pub pulls: Vec<PullConfig>,

    /// Paths to child `tixgraft.yaml` files to run, relative to this file's directory.
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
    /// Reads the YAML file at `path` and validates it against the JSON schema and the rules.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed, or fails validation.
    #[inline]
    pub fn load_from_file(system: &dyn System, path: &str) -> anyhow::Result<Self> {
        yaml::load_config(system, path)
    }

    /// Checks repositories, pulls and child paths, the last against the current directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration breaks one of those rules.
    #[inline]
    pub fn validate(&self, system: &dyn System) -> anyhow::Result<()> {
        validation::validate_config(system, self)
    }
}
