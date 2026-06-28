#![expect(clippy::unwrap_used, reason = "This is a test module")]
#![expect(
    clippy::indexing_slicing,
    reason = "test code uses indexing after length assertions"
)]

use os_shim::{System as _, mock::MockSystem};
use std::path::Path;
use tixgraft::operations::discovery::{cleanup_graft_files, discover_graft_files};

#[test]
fn discover_single_graft() {
    let system = MockSystem::new()
        .with_dir("/test")
        .unwrap()
        .with_file("/test/.graft.yaml", b"# Test graft file\n")
        .unwrap();

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();
    assert_eq!(grafts.len(), 1);
    assert_eq!(grafts[0].depth, 0);
    assert!(grafts[0].parent.is_none());
}

#[test]
fn discover_nested_grafts() {
    let system = MockSystem::new()
        .with_dir("/test")
        .unwrap()
        .with_dir("/test/nested")
        .unwrap()
        .with_dir("/test/nested/deeper")
        .unwrap()
        .with_file("/test/.graft.yaml", b"# Root graft\n")
        .unwrap()
        .with_file("/test/nested/.graft.yaml", b"# Nested graft\n")
        .unwrap()
        .with_file("/test/nested/deeper/.graft.yaml", b"# Deeper graft\n")
        .unwrap();

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
fn cleanup_graft_files_tst() {
    let system = MockSystem::new()
        .with_dir("/test")
        .unwrap()
        .with_dir("/test/nested")
        .unwrap()
        .with_file("/test/.graft.yaml", b"# Root graft\n")
        .unwrap()
        .with_file("/test/nested/.graft.yaml", b"# Nested graft\n")
        .unwrap();

    // Verify files exist
    assert!(system.exists(Path::new("/test/.graft.yaml")).unwrap());
    assert!(
        system
            .exists(Path::new("/test/nested/.graft.yaml"))
            .unwrap()
    );

    // Cleanup
    let deleted = cleanup_graft_files(&system, Path::new("/test")).unwrap();
    assert_eq!(deleted, 2);

    // Verify files are gone
    assert!(!system.exists(Path::new("/test/.graft.yaml")).unwrap());
    assert!(
        !system
            .exists(Path::new("/test/nested/.graft.yaml"))
            .unwrap()
    );
}

#[test]
fn discover_no_grafts() {
    let system = MockSystem::new().with_dir("/test").unwrap();

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();
    assert_eq!(grafts.len(), 0);
}

#[test]
fn discover_nonexistent_directory() {
    let system = MockSystem::new();
    let result = discover_graft_files(&system, Path::new("/nonexistent/path"));
    result.unwrap_err();
}

#[test]
fn discover_not_a_directory() {
    let system = MockSystem::new()
        .with_file("/test/file.txt", b"data")
        .unwrap();

    let result = discover_graft_files(&system, Path::new("/test/file.txt"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not a directory"));
}

#[test]
fn discover_graft_with_non_graft_files() {
    // Ensure non-.graft.yaml files are ignored
    let system = MockSystem::new()
        .with_dir("/test")
        .unwrap()
        .with_file("/test/.graft.yaml", b"# Graft\n")
        .unwrap()
        .with_file("/test/readme.md", b"# Readme\n")
        .unwrap()
        .with_file("/test/config.yaml", b"key: value\n")
        .unwrap();

    let grafts = discover_graft_files(&system, Path::new("/test")).unwrap();
    assert_eq!(grafts.len(), 1);
}

#[test]
fn cleanup_empty_directory() {
    let system = MockSystem::new().with_dir("/test").unwrap();

    let deleted = cleanup_graft_files(&system, Path::new("/test")).unwrap();
    assert_eq!(deleted, 0);
}
