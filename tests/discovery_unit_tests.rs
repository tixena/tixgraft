//! Unit tests for graft file discovery

use std::path::Path;
use tixgraft::operations::discovery::{cleanup_graft_files, discover_graft_files};
use tixgraft::system::{MockSystem, System};

#[test]
fn test_discover_single_graft() {
    let system = MockSystem::new()
        .with_dir("/test")
        .with_file("/test/.graft.yaml", b"# Test graft file\n");

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();
    assert_eq!(grafts.len(), 1);
    assert_eq!(grafts[0].depth, 0);
    assert!(grafts[0].parent.is_none());
}

#[test]
fn test_discover_nested_grafts() {
    let system = MockSystem::new()
        .with_dir("/test")
        .with_dir("/test/nested")
        .with_dir("/test/nested/deeper")
        .with_file("/test/.graft.yaml", b"# Root graft\n")
        .with_file("/test/nested/.graft.yaml", b"# Nested graft\n")
        .with_file("/test/nested/deeper/.graft.yaml", b"# Deeper graft\n");

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();
    assert_eq!(grafts.len(), 3);

    // Check depths
    assert_eq!(grafts[0].depth, 0); // root
    assert_eq!(grafts[1].depth, 1); // nested
    assert_eq!(grafts[2].depth, 2); // doubly nested

    // Check parent relationships
    assert!(grafts[0].parent.is_none());
    assert!(grafts[1].parent.is_some());
    assert!(grafts[2].parent.is_some());

    // Check ancestors
    assert_eq!(grafts[0].ancestors().len(), 0);
    assert_eq!(grafts[1].ancestors().len(), 1);
    assert_eq!(grafts[2].ancestors().len(), 2);
}

#[test]
fn test_cleanup_graft_files() {
    let system = MockSystem::new()
        .with_dir("/test")
        .with_dir("/test/nested")
        .with_file("/test/.graft.yaml", b"# Root graft\n")
        .with_file("/test/nested/.graft.yaml", b"# Nested graft\n");

    // Verify files exist
    assert!(system.exists(Path::new("/test/.graft.yaml")));
    assert!(system.exists(Path::new("/test/nested/.graft.yaml")));

    // Cleanup
    let deleted = cleanup_graft_files(&system, Path::new("/test")).unwrap();
    assert_eq!(deleted, 2);

    // Verify files are gone
    assert!(!system.exists(Path::new("/test/.graft.yaml")));
    assert!(!system.exists(Path::new("/test/nested/.graft.yaml")));
}

#[test]
fn test_discover_no_grafts() {
    let system = MockSystem::new().with_dir("/test");

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();
    assert_eq!(grafts.len(), 0);
}

#[test]
fn test_discover_nonexistent_directory() {
    let system = MockSystem::new();
    let result = discover_graft_files(&system, Path::new("/nonexistent/path"));
    assert!(result.is_err());
}
