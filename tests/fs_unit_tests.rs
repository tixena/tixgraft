//! Unit tests for filesystem utilities.

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

    use os_shim::{System as _, mock::MockSystem};
    use std::path::Path;
    use tixgraft::utils::fs::{
        copy_file_with_progress, create_parent_directories, ensure_dir_exists, format_file_size,
        get_file_size, is_binary_file, is_directory_empty, remove_dir_safe,
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
        assert_eq!(format_file_size(0x0010_0000), "1.0 MB");
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

        // Non-directory returns false
        assert!(!is_directory_empty(&system, Path::new("/test/non_empty/file.txt")).unwrap());
    }

    #[test]
    fn remove_dir_safe_tst() {
        let system = MockSystem::new()
            .with_dir("/test/mydir")
            .unwrap()
            .with_file("/test/mydir/file.txt", b"data")
            .unwrap();

        assert!(system.exists(Path::new("/test/mydir")).unwrap());
        remove_dir_safe(&system, Path::new("/test/mydir")).unwrap();
        assert!(!system.exists(Path::new("/test/mydir")).unwrap());

        // Removing non-existent dir is a no-op
        remove_dir_safe(&system, Path::new("/test/nonexistent")).unwrap();
    }

    #[test]
    fn get_file_size_tst() {
        let system = MockSystem::new()
            .with_file("/test/file.txt", b"hello world")
            .unwrap();

        // MockSystem metadata returns 0 for size, just verify it runs without error
        let size = get_file_size(&system, Path::new("/test/file.txt")).unwrap();
        assert_eq!(size, 0); // MockSystem limitation

        // Non-existent file should error
        get_file_size(&system, Path::new("/test/missing.txt")).unwrap_err();
    }

    #[test]
    fn ensure_dir_exists_tst() {
        let system = MockSystem::new();

        // Creates new directory
        ensure_dir_exists(&system, Path::new("/test/newdir")).unwrap();
        assert!(system.exists(Path::new("/test/newdir")).unwrap());

        // Already exists as dir — no error
        ensure_dir_exists(&system, Path::new("/test/newdir")).unwrap();

        // Exists as file — error
        let system2 = MockSystem::new()
            .with_file("/test/file.txt", b"data")
            .unwrap();
        let result = ensure_dir_exists(&system2, Path::new("/test/file.txt"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a directory"));
    }

    #[test]
    fn copy_file_with_progress_tst() {
        let system = MockSystem::new()
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
        assert!(system.exists(Path::new("/dst/file.txt")).unwrap());
    }

    #[test]
    fn is_binary_file_known_text_extension() {
        let system = MockSystem::new()
            .with_file("/test/code.rs", b"\x80\x81\x82")
            .unwrap();

        // .rs is a known text extension, so it should return false even with binary-like content
        assert!(!is_binary_file(&system, Path::new("/test/code.rs")).unwrap());
    }

    #[test]
    fn is_binary_file_directory() {
        let system = MockSystem::new().with_dir("/test/mydir").unwrap();

        // Directory should return false
        assert!(!is_binary_file(&system, Path::new("/test/mydir")).unwrap());
    }

    #[test]
    fn is_binary_file_empty_file() {
        let system = MockSystem::new().with_file("/test/empty.bin", b"").unwrap();

        // Empty file should be treated as text
        assert!(!is_binary_file(&system, Path::new("/test/empty.bin")).unwrap());
    }

    #[test]
    fn format_file_size_large_values() {
        assert_eq!(format_file_size(0x4000_0000), "1.0 GB");
        assert_eq!(format_file_size(0x100_0000_0000), "1.0 TB");
    }

    #[test]
    fn create_parent_directories_already_exists() {
        let system = MockSystem::new().with_dir("/test/existing").unwrap();

        // Should be a no-op when parent already exists
        create_parent_directories(&system, Path::new("/test/existing/file.txt")).unwrap();
        assert!(system.exists(Path::new("/test/existing")).unwrap());
    }
}
