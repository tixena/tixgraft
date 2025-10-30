//! Unit tests for filesystem utilities


#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

use std::path::Path;
use tixgraft::system::{mock::MockSystem, System as _};
use tixgraft::utils::fs::{
    create_parent_directories, format_file_size, is_binary_file, is_directory_empty,
};

#[test]
fn is_binary_file_tst() {
    let system = MockSystem::new()
        .with_dir("/test").unwrap()
        .with_file("/test/text.txt", b"Hello, world!").unwrap()
        .with_file(
            "/test/utf8.txt",
            "Hello \u{1f3d7}\u{fe0f} World! \u{250c}\u{2500} UTF-8 \u{6587}\u{5b57} \u{627}\u{644}\u{639}\u{631}\u{628}\u{64a}\u{629}".as_bytes(),
        ).unwrap()
        .with_file("/test/binary.bin", &[0, 1, 2, 3, 0xFF, 0xFE]).unwrap();

    let text_file = Path::new("/test/text.txt");
    let utf8_file = Path::new("/test/utf8.txt");
    let binary_file = Path::new("/test/binary.bin");

    assert!(!is_binary_file(&system, text_file).unwrap());
    assert!(!is_binary_file(&system, utf8_file).unwrap());
    assert!(is_binary_file(&system, binary_file).unwrap());
}

#[test]
fn format_file_size_tst() {
    assert_eq!(format_file_size(0), "0 B");
    assert_eq!(format_file_size(1_023), "1023 B");
    assert_eq!(format_file_size(1_024), "1.0 KB");
    assert_eq!(format_file_size(1_536), "1.5 KB");
    assert_eq!(format_file_size(1_048_576), "1.0 MB");
}

#[test]
fn create_parent_directories_tst() {
    let system = MockSystem::new().with_dir("/test").unwrap();

    let nested_file = Path::new("/test/a/b/c/file.txt");

    create_parent_directories(&system, nested_file).unwrap();
    assert!(system.exists(nested_file.parent().unwrap()).unwrap());
}

#[test]
fn is_directory_empty_tst() {
    let system = MockSystem::new()
        .with_dir("/test/empty")
        .unwrap()
        .with_dir("/test/non_empty")
        .unwrap()
        .with_file("/test/non_empty/file.txt", b"content")
        .unwrap();

    let empty_dir = Path::new("/test/empty");
    let non_empty_dir = Path::new("/test/non_empty");

    assert!(is_directory_empty(&system, empty_dir).unwrap());
    assert!(!is_directory_empty(&system, non_empty_dir).unwrap());
}
}