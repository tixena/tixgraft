//! Tests for the filesystem helpers over `MemorySystem`.
//!
//! They cover binary detection, size formatting, directory creation and removal, and copying.

#![expect(clippy::unwrap_used, reason = "This is a test module")]

use os_shim::{System as _, mock::MemorySystem};
use pollster::FutureExt as _;
use std::path::Path;
use tixgraft::utils::fs::{
    copy_file_with_progress, create_parent_directories, ensure_dir_exists, format_file_size,
    get_file_size, is_binary_file, is_directory_empty, remove_dir_safe,
};

#[test]
fn is_binary_file_tst() {
    let system = MemorySystem::new()
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
    assert_eq!(format_file_size(0x0010_0000), "1.0 MB");
}

#[test]
fn create_parent_directories_tst() {
    let system = MemorySystem::new().with_dir("/test").unwrap();

    let nested_file = Path::new("/test/a/b/c/file.txt");

    create_parent_directories(&system, nested_file).unwrap();
    assert!(
        system
            .exists(nested_file.parent().unwrap())
            .block_on()
            .unwrap()
    );
}

#[test]
fn is_directory_empty_tst() {
    let system = MemorySystem::new()
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

    assert!(!is_directory_empty(&system, Path::new("/test/non_empty/file.txt")).unwrap());
}

#[test]
fn remove_dir_safe_tst() {
    let system = MemorySystem::new()
        .with_dir("/test/mydir")
        .unwrap()
        .with_file("/test/mydir/file.txt", b"data")
        .unwrap();

    assert!(system.exists(Path::new("/test/mydir")).block_on().unwrap());
    remove_dir_safe(&system, Path::new("/test/mydir")).unwrap();
    assert!(!system.exists(Path::new("/test/mydir")).block_on().unwrap());

    remove_dir_safe(&system, Path::new("/test/nonexistent")).unwrap();
}

#[test]
fn get_file_size_tst() {
    let system = MemorySystem::new()
        .with_file("/test/file.txt", b"hello world")
        .unwrap();

    let size = get_file_size(&system, Path::new("/test/file.txt")).unwrap();
    assert_eq!(size, 11);

    get_file_size(&system, Path::new("/test/missing.txt")).unwrap_err();
}

#[test]
fn ensure_dir_exists_tst() {
    let system = MemorySystem::new();

    ensure_dir_exists(&system, Path::new("/test/newdir")).unwrap();
    assert!(system.exists(Path::new("/test/newdir")).block_on().unwrap());

    ensure_dir_exists(&system, Path::new("/test/newdir")).unwrap();

    let system2 = MemorySystem::new()
        .with_file("/test/file.txt", b"data")
        .unwrap();
    let result = ensure_dir_exists(&system2, Path::new("/test/file.txt"));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not a directory"));
}

#[test]
fn copy_file_with_progress_tst() {
    let system = MemorySystem::new()
        .with_file("/src/file.txt", b"hello world, this is test data")
        .unwrap();

    let result = copy_file_with_progress(
        &system,
        Path::new("/src/file.txt"),
        Path::new("/dst/file.txt"),
        |_copied, _total| {},
    );

    assert!(result.is_ok());
    assert!(result.unwrap() > 0);
    assert!(
        system
            .exists(Path::new("/dst/file.txt"))
            .block_on()
            .unwrap()
    );
}

/// A known text extension counts as text even when the content looks binary.
#[test]
fn is_binary_file_known_text_extension() {
    let system = MemorySystem::new()
        .with_file("/test/code.rs", b"\x80\x81\x82")
        .unwrap();

    assert!(!is_binary_file(&system, Path::new("/test/code.rs")).unwrap());
}

#[test]
fn is_binary_file_directory() {
    let system = MemorySystem::new().with_dir("/test/mydir").unwrap();

    assert!(!is_binary_file(&system, Path::new("/test/mydir")).unwrap());
}

#[test]
fn is_binary_file_empty_file() {
    let system = MemorySystem::new()
        .with_file("/test/empty.bin", b"")
        .unwrap();

    assert!(!is_binary_file(&system, Path::new("/test/empty.bin")).unwrap());
}

/// `ascii_len` ASCII bytes, then `character`, then more text.
fn straddling_limit(ascii_len: usize, character: char) -> Vec<u8> {
    let mut content = "a".repeat(ascii_len);
    content.push(character);
    content.push_str(" tail");
    content.into_bytes()
}

#[test]
fn is_binary_file_two_byte_character_straddling_limit() {
    let system = MemorySystem::new()
        .with_file("/test/two_byte", &straddling_limit(8_191, '\u{e9}'))
        .unwrap();

    assert!(!is_binary_file(&system, Path::new("/test/two_byte")).unwrap());
}

#[test]
fn is_binary_file_three_byte_character_straddling_limit() {
    let system = MemorySystem::new()
        .with_file(
            "/test/three_byte_first",
            &straddling_limit(8_191, '\u{20ac}'),
        )
        .unwrap()
        .with_file(
            "/test/three_byte_second",
            &straddling_limit(8_190, '\u{20ac}'),
        )
        .unwrap();

    assert!(!is_binary_file(&system, Path::new("/test/three_byte_first")).unwrap());
    assert!(!is_binary_file(&system, Path::new("/test/three_byte_second")).unwrap());
}

#[test]
fn is_binary_file_four_byte_character_straddling_limit() {
    let system = MemorySystem::new()
        .with_file("/test/four_byte", &straddling_limit(8_189, '\u{1f3d7}'))
        .unwrap();

    assert!(!is_binary_file(&system, Path::new("/test/four_byte")).unwrap());
}

/// A full buffer holding a byte that can never start a character stays binary.
#[test]
fn is_binary_file_invalid_byte_in_full_buffer() {
    let mut content = vec![b'a'; 8_191];
    content.extend_from_slice(b"\xff tail");
    let system = MemorySystem::new()
        .with_file("/test/invalid", &content)
        .unwrap();

    assert!(is_binary_file(&system, Path::new("/test/invalid")).unwrap());
}

/// A file shorter than the limit that ends in a cut character stays binary.
#[test]
fn is_binary_file_short_file_ending_in_cut_character() {
    let system = MemorySystem::new()
        .with_file("/test/truncated", b"hello \xe2\x82")
        .unwrap();

    assert!(is_binary_file(&system, Path::new("/test/truncated")).unwrap());
}

#[test]
fn format_file_size_large_values() {
    assert_eq!(format_file_size(0x4000_0000), "1.0 GB");
    assert_eq!(format_file_size(0x100_0000_0000), "1.0 TB");
}

#[test]
fn create_parent_directories_already_exists() {
    let system = MemorySystem::new().with_dir("/test/existing").unwrap();

    create_parent_directories(&system, Path::new("/test/existing/file.txt")).unwrap();
    assert!(
        system
            .exists(Path::new("/test/existing"))
            .block_on()
            .unwrap()
    );
}
