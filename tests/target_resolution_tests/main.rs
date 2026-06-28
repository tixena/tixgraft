//! Integration tests for target path resolution relative to config file directory.
//!
//! These tests verify that relative pull target paths resolve against the
//! config file's parent directory, not the process working directory.

#[cfg(test)]
mod tests;
