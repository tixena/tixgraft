//! Unit tests for Git sparse checkout utilities

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "This is a test module")]
mod tests {

    use tixgraft::git::sparse_checkout::parse_git_version;

    #[test]
    fn parse_git_version_tst() {
        assert_eq!(parse_git_version("2.34.1").unwrap(), (2, 34, 1));
        assert_eq!(parse_git_version("2.25.0").unwrap(), (2, 25, 0));
        parse_git_version("invalid").unwrap_err();
    }
}
