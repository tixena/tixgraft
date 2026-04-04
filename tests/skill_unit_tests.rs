//! Unit tests for skill management operations.

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

    use os_shim::System as _;
    use os_shim::mock::MockSystem;
    use std::path::Path;
    use tixgraft::operations::skill::{SkillStatus, skill_check, skill_install, skill_uninstall};

    fn skill_dir() -> &'static Path {
        Path::new("/project/.claude/skills/tixgraft")
    }

    #[test]
    fn install_creates_files() {
        let system = MockSystem::new();

        skill_install(&system, skill_dir()).unwrap();

        // SKILL.md must exist
        let skill_md = skill_dir().join("SKILL.md");
        assert!(system.exists(&skill_md).unwrap());

        // Content should not be empty
        let content = system.read_to_string(&skill_md).unwrap();
        assert!(content.contains("tixgraft"));
    }

    #[test]
    fn install_overwrites_existing() {
        let system = MockSystem::new()
            .with_dir("/project/.claude/skills/tixgraft")
            .unwrap()
            .with_file("/project/.claude/skills/tixgraft/SKILL.md", b"old content")
            .unwrap();

        skill_install(&system, skill_dir()).unwrap();

        let content = system
            .read_to_string(&skill_dir().join("SKILL.md"))
            .unwrap();
        assert_ne!(content, "old content");
        assert!(content.contains("tixgraft"));
    }

    #[test]
    fn uninstall_removes_directory() {
        let system = MockSystem::new();
        skill_install(&system, skill_dir()).unwrap();

        assert!(system.exists(skill_dir()).unwrap());

        skill_uninstall(&system, skill_dir()).unwrap();

        assert!(!system.exists(skill_dir()).unwrap());
    }

    #[test]
    fn uninstall_idempotent() {
        let system = MockSystem::new();

        // Should not error even if nothing is installed
        let result = skill_uninstall(&system, skill_dir());
        result.unwrap();
    }

    #[test]
    fn check_not_installed() {
        let system = MockSystem::new();

        let status = skill_check(&system, skill_dir()).unwrap();
        assert_eq!(status, SkillStatus::NotInstalled);
    }

    #[test]
    fn check_up_to_date() {
        let system = MockSystem::new();

        skill_install(&system, skill_dir()).unwrap();

        let status = skill_check(&system, skill_dir()).unwrap();
        assert_eq!(status, SkillStatus::UpToDate);
    }

    #[test]
    fn check_outdated_content_differs() {
        let system = MockSystem::new();

        skill_install(&system, skill_dir()).unwrap();

        // Modify the installed file
        system
            .write(&skill_dir().join("SKILL.md"), b"modified content")
            .unwrap();

        let status = skill_check(&system, skill_dir()).unwrap();
        assert_eq!(status, SkillStatus::Outdated);
    }

    #[test]
    fn check_outdated_extra_file() {
        let system = MockSystem::new();

        skill_install(&system, skill_dir()).unwrap();

        // Add an extra file that is not in the embedded content
        system
            .write(&skill_dir().join("extra.md"), b"this should not be here")
            .unwrap();

        let status = skill_check(&system, skill_dir()).unwrap();
        assert_eq!(status, SkillStatus::Outdated);
    }

    #[test]
    fn check_outdated_missing_file() {
        let system = MockSystem::new()
            .with_dir("/project/.claude/skills/tixgraft")
            .unwrap();

        // Directory exists but SKILL.md is missing
        let status = skill_check(&system, skill_dir()).unwrap();
        assert_eq!(status, SkillStatus::Outdated);
    }
}
