//! Unit tests for file and directory copying operations.

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

    use std::path::Path;
    use tixgraft::operations::copy::{
        calculate_copy_size, copy_directory, copy_file, copy_files, count_files_to_copy,
    };
    use tixgraft::system::System as _;
    use tixgraft::system::mock::MockSystem;

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

    #[test]
    fn copy_files_source_not_found() {
        let system = MockSystem::new();

        let result = copy_files(
            &system,
            Path::new("/nonexistent/source"),
            "/target",
            "file",
            false,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn copy_files_invalid_type() {
        let system = MockSystem::new()
            .with_file("/test/source.txt", b"content")
            .unwrap();

        let result = copy_files(
            &system,
            Path::new("/test/source.txt"),
            "/target",
            "invalid_type",
            false,
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid pull type")
        );
    }

    #[test]
    fn copy_files_file_type() {
        let system = MockSystem::new()
            .with_file("/test/source.txt", b"hello")
            .unwrap();

        let result = copy_files(
            &system,
            Path::new("/test/source.txt"),
            "/target/out.txt",
            "file",
            false,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
        assert!(system.exists(Path::new("/target/out.txt")).unwrap());
    }

    #[test]
    fn copy_files_directory_with_reset() {
        let system = MockSystem::new()
            .with_dir("/source")
            .unwrap()
            .with_file("/source/a.txt", b"new content")
            .unwrap()
            .with_dir("/target")
            .unwrap()
            .with_file("/target/old.txt", b"old content")
            .unwrap();

        let result = copy_files(&system, Path::new("/source"), "/target", "directory", true);
        assert!(result.is_ok());
        // Old file should be gone after reset
        assert!(!system.exists(Path::new("/target/old.txt")).unwrap());
        assert!(system.exists(Path::new("/target/a.txt")).unwrap());
    }

    #[test]
    fn copy_file_source_not_file() {
        let system = MockSystem::new().with_dir("/test/source").unwrap();

        let result = copy_file(&system, Path::new("/test/source"), Path::new("/target"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a file"));
    }

    #[test]
    fn copy_directory_source_not_dir() {
        let system = MockSystem::new()
            .with_file("/test/source.txt", b"data")
            .unwrap();

        let result = copy_directory(&system, Path::new("/test/source.txt"), Path::new("/target"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a directory"));
    }

    #[test]
    fn calculate_copy_size_file() {
        let system = MockSystem::new()
            .with_file("/test/file.txt", b"12345")
            .unwrap();

        // MockSystem metadata returns 0 for size, just verify it runs
        let size = calculate_copy_size(&system, Path::new("/test/file.txt"), "file").unwrap();
        assert_eq!(size, 0); // MockSystem limitation
    }

    #[test]
    fn calculate_copy_size_directory() {
        let system = MockSystem::new()
            .with_dir("/test/dir")
            .unwrap()
            .with_file("/test/dir/a.txt", b"aaa")
            .unwrap()
            .with_file("/test/dir/b.txt", b"bbbbb")
            .unwrap();

        // MockSystem metadata returns 0 for size, just verify it runs
        let size = calculate_copy_size(&system, Path::new("/test/dir"), "directory").unwrap();
        assert_eq!(size, 0); // MockSystem limitation
    }

    #[test]
    fn calculate_copy_size_invalid_type() {
        let system = MockSystem::new()
            .with_file("/test/file.txt", b"data")
            .unwrap();

        let size = calculate_copy_size(&system, Path::new("/test/file.txt"), "unknown").unwrap();
        assert_eq!(size, 0);
    }

    #[test]
    fn count_files_to_copy_file() {
        let system = MockSystem::new()
            .with_file("/test/file.txt", b"data")
            .unwrap();

        assert_eq!(
            count_files_to_copy(&system, Path::new("/test/file.txt"), "file").unwrap(),
            1
        );
    }

    #[test]
    fn count_files_to_copy_directory() {
        let system = MockSystem::new()
            .with_dir("/test/dir")
            .unwrap()
            .with_file("/test/dir/a.txt", b"a")
            .unwrap()
            .with_file("/test/dir/b.txt", b"b")
            .unwrap()
            .with_file("/test/dir/c.txt", b"c")
            .unwrap();

        assert_eq!(
            count_files_to_copy(&system, Path::new("/test/dir"), "directory").unwrap(),
            3
        );
    }

    #[test]
    fn count_files_to_copy_invalid_type() {
        let system = MockSystem::new();

        assert_eq!(
            count_files_to_copy(&system, Path::new("/whatever"), "invalid").unwrap(),
            0
        );
    }
}
