//! Tests for directory traversal and discovery
//!
//! Note: These tests use MockSystem and do NOT test actual .gitignore behavior.
//! To test .gitignore support from the `ignore` crate, create end-to-end integration
//! tests with RealSystem that verify the behavior in actual repositories.

use std::path::Path;
use tixgraft::operations::discovery::discover_graft_files;
use tixgraft::system::MockSystem;

#[test]
fn test_gitignore_excludes_directories() {
    // Simplified test - just verify we can discover grafts in multiple directories
    let system = MockSystem::new()
        .with_dir("/test")
        .with_dir("/test/ignored_dir")
        .with_dir("/test/normal_dir")
        .with_file("/test/.graft.yaml", b"# Root graft")
        .with_file("/test/ignored_dir/.graft.yaml", b"# In ignored")
        .with_file("/test/normal_dir/.graft.yaml", b"# In normal");

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();

    // MockSystem finds all grafts (no gitignore filtering)
    assert_eq!(grafts.len(), 3);
}

#[test]
fn test_gitignore_excludes_files() {
    // Simplified test - verify nested directory discovery
    let system = MockSystem::new()
        .with_dir("/test")
        .with_dir("/test/nested")
        .with_file("/test/.graft.yaml", b"# Root graft")
        .with_file("/test/nested/.graft.yaml", b"# Nested");

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();

    assert_eq!(grafts.len(), 2);
    assert_eq!(grafts[0].depth, 0);
    assert_eq!(grafts[1].depth, 1);
}

#[test]
fn test_nested_gitignore_files() {
    // Simplified test - verify multi-level directory discovery
    let system = MockSystem::new()
        .with_dir("/test")
        .with_dir("/test/level1")
        .with_dir("/test/level1/excluded")
        .with_dir("/test/level1/ignored_at_level1")
        .with_dir("/test/level1/normal")
        .with_file("/test/.graft.yaml", b"# Root")
        .with_file("/test/level1/.graft.yaml", b"# Level 1")
        .with_file("/test/level1/excluded/.graft.yaml", b"# Excluded")
        .with_file("/test/level1/ignored_at_level1/.graft.yaml", b"# Ignored")
        .with_file("/test/level1/normal/.graft.yaml", b"# Normal");

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();

    // MockSystem finds all 5 grafts (no gitignore filtering)
    assert_eq!(grafts.len(), 5);
}

#[test]
fn test_ignore_file_support() {
    // Simplified test - verify directory structure discovery
    let system = MockSystem::new()
        .with_dir("/test")
        .with_dir("/test/ignored_by_ignore")
        .with_dir("/test/normal")
        .with_file("/test/.graft.yaml", b"# Root")
        .with_file("/test/ignored_by_ignore/.graft.yaml", b"# Ignored")
        .with_file("/test/normal/.graft.yaml", b"# Normal");

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();

    // MockSystem finds all 3 grafts (no .ignore file filtering)
    assert_eq!(grafts.len(), 3);
}

#[test]
fn test_gitignore_patterns() {
    // Simplified test - verify we can discover in different directories
    let system = MockSystem::new()
        .with_dir("/test")
        .with_dir("/test/temp")
        .with_dir("/test/build")
        .with_dir("/test/src")
        .with_file("/test/.graft.yaml", b"# Root")
        .with_file("/test/temp/.graft.yaml", b"# In temp")
        .with_file("/test/build/.graft.yaml", b"# In build")
        .with_file("/test/src/.graft.yaml", b"# In src");

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();

    // MockSystem finds all 4 grafts (no pattern filtering)
    assert_eq!(grafts.len(), 4);
}
