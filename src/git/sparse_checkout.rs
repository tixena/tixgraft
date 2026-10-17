//! Git path-scoped checkout implementation, backed by libgit2.

use crate::error::GraftError;
use crate::git::Repository;
use anyhow::{Context as _, Result};
use core::fmt::Write as _;
use core::sync::atomic::{AtomicUsize, Ordering};
use git2::build::CheckoutBuilder;
use git2::{
    Config as GitConfig, Cred, CredentialType, Direction, Error as GitError, FetchOptions, Oid,
    Remote, RemoteCallbacks, RemoteHead, Repository as GitRepository, Version,
};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tracing::debug;

/// Ceiling on credential callback re-entry, so a rejected key cannot spin forever.
const MAX_CREDENTIAL_ATTEMPTS: usize = 4;

/// Local ref the requested reference is fetched into, whatever its kind upstream.
const FETCHED_REF: &str = "refs/tixgraft/fetched";

/// Performs a path-scoped checkout of a specific path from a Git repository.
#[non_exhaustive]
pub struct SparseCheckout {
    /// Git reference (tag/branch) to checkout.
    pub reference: String,
    /// Repository to clone from.
    pub repository: Repository,
    /// Path within the repository to sparse-checkout.
    pub source_path: String,
    /// Temporary directory for the sparse checkout.
    pub temp_dir: TempDir,
}

impl SparseCheckout {
    /// Write only `source_path` out of `commit_id` into the working tree.
    fn checkout_source_path(&self, repo: &GitRepository, commit_id: Oid) -> Result<()> {
        let commit = repo.find_commit(commit_id).map_err(|err| {
            GraftError::git(format!(
                "Failed to read commit {commit_id} from '{}': {}",
                self.repository.original_url(),
                err.message()
            ))
        })?;

        let mut checkout = CheckoutBuilder::new();
        checkout.force();
        // A directory source needs the trailing glob; a file source matches on its own.
        checkout.path(&self.source_path);
        checkout.path(format!("{}/*", self.source_path));

        repo.checkout_tree(commit.as_object(), Some(&mut checkout))
            .map_err(|err| {
                GraftError::git(format!(
                    "Failed to check out '{}' at reference '{}': {}",
                    self.source_path,
                    self.reference,
                    err.message()
                ))
            })?;

        repo.set_head_detached(commit_id).map_err(|err| {
            GraftError::git(format!(
                "Failed to point HEAD at {commit_id}: {}",
                err.message()
            ))
        })?;

        Ok(())
    }

    /// Execute the path-scoped checkout operation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The temporary repository cannot be initialized
    /// - The repository cannot be reached or authenticated against
    /// - The reference cannot be resolved to a commit
    /// - The requested path cannot be checked out
    #[inline]
    pub fn execute(&self) -> Result<PathBuf> {
        let repo_path = self.temp_dir.path();

        debug!("Executing path-scoped checkout operation: {repo_path:?}");

        let repo = GitRepository::init(repo_path).map_err(|err| {
            GraftError::git(format!(
                "Failed to initialize temporary repository at {}: {}",
                repo_path.display(),
                err.message()
            ))
        })?;

        debug!("Temporary repository initialized, fetching reference");

        let commit_id = self.fetch_commit(&repo)?;

        debug!("Reference resolved to {commit_id}, checking out requested path");

        self.checkout_source_path(&repo, commit_id)?;

        let result_path = repo_path.join(&self.source_path);
        debug!("Returning path to checked out source: {result_path:?}");

        Ok(result_path)
    }

    /// Fetch the requested reference and return the commit it points at.
    fn fetch_commit(&self, repo: &GitRepository) -> Result<Oid> {
        let url = self.repository.git_url()?;
        let mut remote = repo.remote_anonymous(url).map_err(|err| {
            GraftError::git(format!(
                "Failed to address repository '{}': {}",
                self.repository.original_url(),
                err.message()
            ))
        })?;

        let refspec = self.resolve_refspec(&mut remote)?;

        let mut options = FetchOptions::new();
        options.remote_callbacks(remote_callbacks());

        if let Some(spec) = refspec {
            // A known branch or tag needs only its own tip.
            options.depth(1_i32);
            remote
                .fetch(&[spec], Some(&mut options), None)
                .map_err(|err| self.fetch_error(&err))?;

            let commit = repo
                .find_reference(FETCHED_REF)
                .and_then(|reference| reference.peel_to_commit())
                .map_err(|err| {
                    GraftError::git(format!(
                        "Failed to resolve reference '{}' after fetch: {}",
                        self.reference,
                        err.message()
                    ))
                })?;

            Ok(commit.id())
        } else {
            // Neither a branch nor a tag, so it may be a commit id or a revision
            // expression; those need real history to resolve against.
            remote
                .fetch(
                    &[
                        "+refs/heads/*:refs/remotes/origin/*",
                        "+refs/tags/*:refs/tags/*",
                    ],
                    Some(&mut options),
                    None,
                )
                .map_err(|err| self.fetch_error(&err))?;

            let commit = repo
                .revparse_single(&self.reference)
                .and_then(|object| object.peel_to_commit())
                .map_err(|err| {
                    GraftError::git(format!(
                        "Failed to resolve reference '{}' in repository '{}': {}",
                        self.reference,
                        self.repository.original_url(),
                        err.message()
                    ))
                })?;

            Ok(commit.id())
        }
    }

    /// Describe a failed fetch against this repository.
    fn fetch_error(&self, err: &GitError) -> GraftError {
        GraftError::git(format!(
            "Failed to fetch '{}' from repository '{}': {}",
            self.reference,
            self.repository.original_url(),
            err.message()
        ))
    }

    /// Get diagnostic information about what was actually checked out.
    /// This is useful for debugging when `source_exists()` returns false.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory cannot be read
    #[inline]
    pub fn get_checkout_diagnostics(&self) -> Result<String> {
        use std::fs;
        let repo_path = self.temp_dir.path();
        let mut diagnostics = String::new();

        writeln!(
            diagnostics,
            "Checkout diagnostics:\n  Repository: {}\n  Reference: {}\n  Requested path: {}\n",
            self.repository.original_url(),
            self.reference,
            self.source_path
        )?;

        writeln!(diagnostics, "  Temp directory: {}", repo_path.display())?;

        // List what was actually checked out
        diagnostics.push_str("  Checked out files:\n");
        if let Ok(entries) = fs::read_dir(repo_path) {
            let mut found_items = Vec::new();
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    // Skip .git directory
                    if file_name != ".git" {
                        found_items.push(file_name);
                    }
                }
            }

            if found_items.is_empty() {
                writeln!(diagnostics, "    (empty - no files were checked out)")?;
            } else {
                for item in found_items {
                    writeln!(diagnostics, "    - {item}")?;
                }
            }
        } else {
            writeln!(diagnostics, "    (unable to read directory)")?;
        }

        diagnostics.push_str("  Checkout path filter:\n");
        writeln!(diagnostics, "    {}", self.source_path)?;
        writeln!(diagnostics, "    {}/*", self.source_path)?;

        Ok(diagnostics)
    }

    /// Create a new path-scoped checkout operation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The temporary directory cannot be created
    #[inline]
    pub fn new(repository: Repository, reference: String, source_path: String) -> Result<Self> {
        let temp_dir =
            TempDir::new().context("Failed to create temporary directory for Git operations")?;

        Ok(Self {
            reference,
            repository,
            source_path,
            temp_dir,
        })
    }

    /// Find the refspec that fetches the requested reference, if the remote publishes
    /// it as a branch or a tag.
    fn resolve_refspec(&self, remote: &mut Remote<'_>) -> Result<Option<String>> {
        let connection = remote
            .connect_auth(Direction::Fetch, Some(remote_callbacks()), None)
            .map_err(|err| {
                GraftError::git(format!(
                    "Failed to connect to repository '{}': {}",
                    self.repository.original_url(),
                    err.message()
                ))
            })?;

        let heads = connection.list().map_err(|err| {
            GraftError::git(format!(
                "Failed to list references of repository '{}': {}",
                self.repository.original_url(),
                err.message()
            ))
        })?;

        let branch_ref = format!("refs/heads/{}", self.reference);
        let tag_ref = format!("refs/tags/{}", self.reference);

        let refspec = heads
            .iter()
            .map(RemoteHead::name)
            .find(|name| *name == branch_ref || *name == tag_ref)
            .map(|name| format!("+{name}:{FETCHED_REF}"));

        Ok(refspec)
    }

    /// Check if the source path exists after checkout.
    #[must_use]
    #[inline]
    pub fn source_exists(&self) -> bool {
        self.temp_dir.path().join(&self.source_path).exists()
    }

    /// Get the path to the temporary directory.
    #[must_use]
    #[inline]
    pub fn temp_path(&self) -> &Path {
        self.temp_dir.path()
    }
}

/// Check that the linked libgit2 carries the transports remote pulls need.
///
/// # Errors
///
/// Returns an error if:
/// - libgit2 was built without HTTPS support
/// - libgit2 was built without SSH support
#[inline]
pub fn check_git_support() -> Result<()> {
    let version = Version::get();

    if !version.https() {
        return Err(GraftError::git(
            "The linked libgit2 has no HTTPS support, so https:// repositories cannot be fetched"
                .to_owned(),
        )
        .into());
    }

    if !version.ssh() {
        return Err(GraftError::git(
            "The linked libgit2 has no SSH support, so git@ repositories cannot be fetched"
                .to_owned(),
        )
        .into());
    }

    Ok(())
}

/// Build the callbacks libgit2 uses to authenticate against a remote.
///
/// SSH keys come from the agent and HTTPS credentials from the configured git
/// credential helper, so tixgraft honours whatever the user already set up.
fn remote_callbacks() -> RemoteCallbacks<'static> {
    let attempts = AtomicUsize::new(0);
    let mut callbacks = RemoteCallbacks::new();

    callbacks.credentials(move |url, username_from_url, allowed| {
        if attempts.fetch_add(1, Ordering::Relaxed) >= MAX_CREDENTIAL_ATTEMPTS {
            return Err(GitError::from_str(
                "every available credential was rejected by the remote",
            ));
        }

        if allowed.contains(CredentialType::SSH_KEY) {
            Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
        } else if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
            let config = GitConfig::open_default()?;
            Cred::credential_helper(&config, url, username_from_url)
        } else {
            Cred::default()
        }
    });

    callbacks
}
