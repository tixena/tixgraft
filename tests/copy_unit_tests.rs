//! Unit tests for file and directory copying operations



#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

use std::path::Path;
use tixgraft::operations::copy::{copy_directory, copy_file};
use tixgraft::system::mock::MockSystem;
use tixgraft::system::System as _;

#[test]
fn copy_file_tst() {
    let system = MockSystem::new()
        .with_dir("/test")
        .unwrap()
        .with_file("/test/source.txt", b"test content\n")
        .unwrap();

    let source_path = Path::new("/test/source.txt");
    let target_path = Path::new("/test/target.txt");

    // Copy file
    let result = copy_file(&system, source_path, target_path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1);

    // Verify target exists and has correct content
    assert!(system.exists(target_path).unwrap());
    let content = system.read_to_string(target_path).unwrap();
    assert_eq!(content.trim(), "test content");
}

#[test]
fn copy_directory_tst() {
    let system = MockSystem::new()
        .with_dir("/test/source")
        .unwrap()
        .with_dir("/test/source/subdir")
        .unwrap()
        .with_file("/test/source/file1.txt", b"content1\n")
        .unwrap()
        .with_file("/test/source/subdir/file2.txt", b"content2\n")
        .unwrap();

    let source_dir = Path::new("/test/source");
    let target_dir = Path::new("/test/target");

    // Copy directory
    let result = copy_directory(&system, source_dir, target_dir);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 2);

    // Verify target structure
    assert!(system.exists(&target_dir.join("file1.txt")).unwrap());
    assert!(system.exists(&target_dir.join("subdir/file2.txt")).unwrap());
}
}