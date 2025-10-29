//! Unit tests for file and directory copying operations

use std::path::Path;
use tixgraft::operations::copy::{copy_directory, copy_file};
use tixgraft::system::{MockSystem, System};

#[test]
fn test_copy_file() {
    let system = MockSystem::new()
        .with_dir("/test")
        .with_file("/test/source.txt", b"test content\n");

    let source_path = Path::new("/test/source.txt");
    let target_path = Path::new("/test/target.txt");

    // Copy file
    let result = copy_file(&system, source_path, target_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // Verify target exists and has correct content
    assert!(system.exists(target_path));
    let content = system.read_to_string(target_path).unwrap();
    assert_eq!(content.trim(), "test content");
}

#[test]
fn test_copy_directory() {
    let system = MockSystem::new()
        .with_dir("/test/source")
        .with_dir("/test/source/subdir")
        .with_file("/test/source/file1.txt", b"content1\n")
        .with_file("/test/source/subdir/file2.txt", b"content2\n");

    let source_dir = Path::new("/test/source");
    let target_dir = Path::new("/test/target");

    // Copy directory
    let result = copy_directory(&system, source_dir, target_dir);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 2);

    // Verify target structure
    assert!(system.exists(&target_dir.join("file1.txt")));
    assert!(system.exists(&target_dir.join("subdir/file2.txt")));
}
