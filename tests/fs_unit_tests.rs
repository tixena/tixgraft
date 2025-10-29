//! Unit tests for filesystem utilities

use std::path::Path;
use tixgraft::system::{MockSystem, System};
use tixgraft::utils::fs::{
    create_parent_directories, format_file_size, is_binary_file, is_directory_empty,
};

#[test]
fn test_is_binary_file() {
    let system = MockSystem::new()
        .with_dir("/test")
        .with_file("/test/text.txt", b"Hello, world!")
        .with_file(
            "/test/utf8.txt",
            "Hello 🏗️ World! ┌─ UTF-8 文字 العربية".as_bytes(),
        )
        .with_file("/test/binary.bin", &[0, 1, 2, 3, 0xFF, 0xFE]);

    let text_file = Path::new("/test/text.txt");
    let utf8_file = Path::new("/test/utf8.txt");
    let binary_file = Path::new("/test/binary.bin");

    assert!(!is_binary_file(&system, text_file).unwrap());
    assert!(!is_binary_file(&system, utf8_file).unwrap());
    assert!(is_binary_file(&system, binary_file).unwrap());
}

#[test]
fn test_format_file_size() {
    assert_eq!(format_file_size(0), "0 B");
    assert_eq!(format_file_size(1023), "1023 B");
    assert_eq!(format_file_size(1024), "1.0 KB");
    assert_eq!(format_file_size(1536), "1.5 KB");
    assert_eq!(format_file_size(1048576), "1.0 MB");
}

#[test]
fn test_create_parent_directories() {
    let system = MockSystem::new().with_dir("/test");

    let nested_file = Path::new("/test/a/b/c/file.txt");

    assert!(create_parent_directories(&system, nested_file).is_ok());
    assert!(system.exists(nested_file.parent().unwrap()));
}

#[test]
fn test_is_directory_empty() {
    let system = MockSystem::new()
        .with_dir("/test/empty")
        .with_dir("/test/non_empty")
        .with_file("/test/non_empty/file.txt", b"content");

    let empty_dir = Path::new("/test/empty");
    let non_empty_dir = Path::new("/test/non_empty");

    assert!(is_directory_empty(&system, empty_dir).unwrap());
    assert!(!is_directory_empty(&system, non_empty_dir).unwrap());
}
