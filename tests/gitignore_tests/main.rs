//! Tests for directory traversal and discovery.
//!
//! Note: These tests use `MockSystem` and do NOT test actual .gitignore behavior.
//! To test .gitignore support from the `ignore` crate, create end-to-end integration
//! tests with `RealSystem` that verify the behavior in actual repositories.

#[cfg(test)]
mod tests;
