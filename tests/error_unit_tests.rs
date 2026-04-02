//! Unit tests for error types.

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {
    use tixgraft::error::GraftError;

    #[test]
    fn constructors_produce_correct_variants() {
        let cmd = GraftError::command("cmd fail");
        assert!(matches!(cmd, GraftError::Command { .. }));
        assert_eq!(cmd.to_string(), "Command error: cmd fail");

        let cfg = GraftError::configuration("bad config");
        assert!(matches!(cfg, GraftError::Configuration { .. }));
        assert_eq!(cfg.to_string(), "Configuration error: bad config");

        let fs = GraftError::filesystem("io fail");
        assert!(matches!(fs, GraftError::Filesystem { .. }));
        assert_eq!(fs.to_string(), "Filesystem error: io fail");

        let src = GraftError::from_source("not found");
        assert!(matches!(src, GraftError::Source { .. }));
        assert_eq!(src.to_string(), "Source error: not found");

        let git = GraftError::git("clone fail");
        assert!(matches!(git, GraftError::Git { .. }));
        assert_eq!(git.to_string(), "Git error: clone fail");

        let skill = GraftError::skill("install fail");
        assert!(matches!(skill, GraftError::Skill { .. }));
        assert_eq!(skill.to_string(), "Skill error: install fail");
    }

    #[test]
    fn exit_codes() {
        assert_eq!(GraftError::configuration("x").exit_code(), 1);
        assert_eq!(GraftError::from_source("x").exit_code(), 2);
        assert_eq!(GraftError::command("x").exit_code(), 3);
        assert_eq!(GraftError::git("x").exit_code(), 4);
        assert_eq!(GraftError::filesystem("x").exit_code(), 5);
        assert_eq!(GraftError::skill("x").exit_code(), 6);
    }

    #[test]
    fn accepts_string_and_str() {
        // &str
        let _ = GraftError::command("static str");
        // String
        let _ = GraftError::command(String::from("owned string"));
        // format!
        let _ = GraftError::command(format!("formatted {}", 42));
    }
}
