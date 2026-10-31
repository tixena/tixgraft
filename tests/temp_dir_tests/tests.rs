#![expect(clippy::unwrap_used, reason = "This is a test module")]

use os_shim::System as _;
use os_shim::bucket::Condition;
use os_shim::mock::MemorySystem;
use pollster::FutureExt as _;

#[test]
fn mock_temp_dir_creation() {
    let system = MemorySystem::new();

    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let temp_path = temp_dir.path();

    // Verify the temp directory exists
    assert!(system.exists(temp_path).block_on().unwrap());
    assert!(system.is_dir(temp_path).block_on().unwrap());

    // Create a file in the temp directory
    let file_path = temp_path.join("test.txt");
    system
        .write(&file_path, b"test content", Condition::None)
        .block_on()
        .unwrap();
    assert!(system.exists(&file_path).block_on().unwrap());
}

#[test]
fn mock_temp_dir_cleanup_on_drop() {
    let system = MemorySystem::new();
    let temp_path = {
        let temp_dir = system.create_temp_dir().block_on().unwrap();
        let path = temp_dir.path().to_path_buf();

        // Create files in the temp directory
        system
            .write(&path.join("file1.txt"), b"content1", Condition::None)
            .block_on()
            .unwrap();
        system
            .write(&path.join("file2.txt"), b"content2", Condition::None)
            .block_on()
            .unwrap();

        // Verify files exist
        assert!(system.exists(&path.join("file1.txt")).block_on().unwrap());
        assert!(system.exists(&path.join("file2.txt")).block_on().unwrap());

        path
        // temp_dir is dropped here
    };

    // After drop, temp directory and its contents should be gone
    assert!(!system.exists(&temp_path).block_on().unwrap());
    assert!(
        !system
            .exists(&temp_path.join("file1.txt"))
            .block_on()
            .unwrap()
    );
    assert!(
        !system
            .exists(&temp_path.join("file2.txt"))
            .block_on()
            .unwrap()
    );
}

#[test]
fn mock_multiple_temp_dirs() {
    let system = MemorySystem::new();

    let temp1 = system.create_temp_dir().block_on().unwrap();
    let temp2 = system.create_temp_dir().block_on().unwrap();
    let temp3 = system.create_temp_dir().block_on().unwrap();

    // All temp directories should have unique paths
    assert_ne!(temp1.path(), temp2.path());
    assert_ne!(temp2.path(), temp3.path());
    assert_ne!(temp1.path(), temp3.path());

    // All should exist
    assert!(system.exists(temp1.path()).block_on().unwrap());
    assert!(system.exists(temp2.path()).block_on().unwrap());
    assert!(system.exists(temp3.path()).block_on().unwrap());
}

#[test]
fn temp_dir_with_subdirectories() {
    let system = MemorySystem::new();
    let temp_dir = system.create_temp_dir().block_on().unwrap();
    let temp_path = temp_dir.path();

    // Create nested directory structure
    let subdir = temp_path.join("subdir");
    system.create_dir_all(&subdir).block_on().unwrap();
    system
        .write(
            &subdir.join("nested.txt"),
            b"nested content",
            Condition::None,
        )
        .block_on()
        .unwrap();

    assert!(system.exists(&subdir).block_on().unwrap());
    assert!(
        system
            .exists(&subdir.join("nested.txt"))
            .block_on()
            .unwrap()
    );

    // Keep temp_path for verification after drop
    let path_copy = temp_path.to_path_buf();
    drop(temp_dir);

    // Everything should be cleaned up
    assert!(!system.exists(&path_copy).block_on().unwrap());
    assert!(!system.exists(&path_copy.join("subdir")).block_on().unwrap());
}
