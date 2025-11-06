mod docker;
mod github;
mod test_util;

use crate::deps::docker::Docker;
use crate::deps::github::branch::GitHubBranch;
use crate::deps::github::release::GitHubRelease;
use crate::error::Error;
use crate::util::ParsingContext;
use async_trait::async_trait;
use enum_as_inner::EnumAsInner;
use rnix::{SyntaxKind, SyntaxNode};
use serde::{Deserialize, Serialize as SerdeSerialize};
use std::collections::BTreeMap;
use std::fs;

#[derive(EnumAsInner, Clone, Debug)]
pub enum Dependency {
    Docker(Docker),
    GitHubBranch(GitHubBranch),
    GitHubRelease(GitHubRelease),
}

#[derive(Clone, Debug, SerdeSerialize, Deserialize)]
pub struct DependencyMetadata {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_version: Option<String>, // What user specified: "latest", "stable", "15", "main"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_version: Option<String>, // Technical identifier: SHA, digest, or release tag
    pub dep_type: String,
    pub description: String,
}

impl DependencyMetadata {
    /// Returns a friendly display string for the dependency type,
    /// including relevant selector information where appropriate.
    /// This is computed on-demand and not stored in the lock file.
    pub fn type_display(&self, lock_entry: &LockEntry) -> Result<String, Error> {
        let dep = Dependency::from_lock_entry(lock_entry)?;
        Ok(dep.type_display())
    }

    /// Returns a human-friendly version string for display.
    /// This is computed on-demand and not stored in the lock file.
    pub fn friendly_version_display(&self, lock_entry: &LockEntry) -> Result<String, Error> {
        let resolved = self.resolved_version.as_deref().unwrap_or("pending");
        let dep = Dependency::from_lock_entry(lock_entry)?;
        Ok(dep.friendly_version(resolved))
    }
}

impl Dependency {
    /// Returns a friendly display string for the dependency type.
    pub fn type_display(&self) -> String {
        match self {
            Dependency::Docker(d) => d.type_display(),
            Dependency::GitHubBranch(d) => d.type_display(),
            Dependency::GitHubRelease(d) => d.type_display(),
        }
    }

    /// Returns a human-friendly version string for display.
    pub fn friendly_version(&self, resolved_version: &str) -> String {
        match self {
            Dependency::Docker(d) => d.friendly_version(resolved_version),
            Dependency::GitHubBranch(d) => d.friendly_version(resolved_version),
            Dependency::GitHubRelease(d) => d.friendly_version(resolved_version),
        }
    }
}

#[derive(Clone, Debug, SerdeSerialize, Deserialize)]
pub struct LockEntry {
    pub metadata: DependencyMetadata,
    pub lock: serde_json::Value,
}

// Type alias for the entire lock file
pub type LockFile = BTreeMap<String, LockEntry>;

#[async_trait]
pub trait Lockable {
    fn key(&self) -> String;
    fn matches(&self, pattern: &str) -> bool;
    async fn lock_with_metadata(&self) -> Result<LockEntry, Error>;

    /// Returns a friendly display string for the dependency type,
    /// including relevant selector information where appropriate.
    /// For example: "docker-image (15)", "github-branch (main)", "github-release"
    fn type_display(&self) -> String;

    /// Returns a human-friendly version string for display.
    /// Takes the resolved version and formats it appropriately for the dependency type.
    /// For example: shortens SHA digests, truncates commit hashes, etc.
    fn friendly_version(&self, resolved_version: &str) -> String;
}

impl Dependency {
    pub fn new(
        context: &ParsingContext,
        func: &str,
        node: &SyntaxNode,
    ) -> Result<Option<Dependency>, Error> {
        match func {
            "uptix.dockerImage" => Ok(Some(Dependency::Docker(Docker::new(context, &node)?))),
            "uptix.githubBranch" => Ok(Some(Dependency::GitHubBranch(GitHubBranch::new(
                context, &node,
            )?))),
            "uptix.githubRelease" => Ok(Some(Dependency::GitHubRelease(GitHubRelease::new(
                context, &node,
            )?))),
            _ => Ok(None),
        }
    }

    /// Reconstructs a Dependency from a lock entry by dispatching to the
    /// appropriate dependency type based on dep_type.
    pub fn from_lock_entry(entry: &LockEntry) -> Result<Dependency, Error> {
        match entry.metadata.dep_type.as_str() {
            "docker" => Docker::from_lock_entry(entry)
                .map(Dependency::Docker)
                .ok_or_else(|| {
                    Error::StringError(format!(
                        "Failed to reconstruct Docker dependency from lock entry"
                    ))
                }),
            "github-branch" => GitHubBranch::from_lock_entry(entry)
                .map(Dependency::GitHubBranch)
                .ok_or_else(|| {
                    Error::StringError(format!(
                        "Failed to reconstruct GitHubBranch dependency from lock entry"
                    ))
                }),
            "github-release" => GitHubRelease::from_lock_entry(entry)
                .map(Dependency::GitHubRelease)
                .ok_or_else(|| {
                    Error::StringError(format!(
                        "Failed to reconstruct GitHubRelease dependency from lock entry"
                    ))
                }),
            unknown => Err(Error::StringError(format!(
                "Unknown dependency type: {}",
                unknown
            ))),
        }
    }

    pub fn key(&self) -> String {
        match self {
            Dependency::Docker(d) => d.key(),
            Dependency::GitHubBranch(d) => d.key(),
            Dependency::GitHubRelease(d) => d.key(),
        }
    }

    pub fn matches(&self, pattern: &str) -> bool {
        match self {
            Dependency::Docker(d) => d.matches(pattern),
            Dependency::GitHubBranch(d) => d.matches(pattern),
            Dependency::GitHubRelease(d) => d.matches(pattern),
        }
    }

    pub async fn lock_with_metadata(&self) -> Result<LockEntry, Error> {
        match self {
            Dependency::Docker(d) => d.lock_with_metadata().await,
            Dependency::GitHubBranch(d) => d.lock_with_metadata().await,
            Dependency::GitHubRelease(d) => d.lock_with_metadata().await,
        }
    }
}

pub fn collect_file_dependencies(file_path: &str) -> Result<Vec<Dependency>, Error> {
    let content = fs::read_to_string(file_path).unwrap();
    let ast = rnix::parse(&content);
    let context = ParsingContext::new(file_path, &content);
    return collect_ast_dependencies(&context, ast.node());
}

fn collect_ast_dependencies(
    context: &ParsingContext,
    node: SyntaxNode,
) -> Result<Vec<Dependency>, Error> {
    if node.kind() != SyntaxKind::NODE_SELECT {
        return node
            .children()
            .map(|n| collect_ast_dependencies(&context, n))
            .try_fold(Vec::new(), |mut acc, next| {
                acc.extend_from_slice(&next?);
                Ok(acc)
            });
    }

    let func = node.text().to_string();
    if !func.starts_with("uptix.") {
        return Ok(vec![]);
    }

    let value_node = node.next_sibling();
    if value_node.is_none() {
        return Ok(vec![]);
    }

    return match <Dependency>::new(&context, &func, &value_node.unwrap())? {
        Some(dependency) => Ok(vec![dependency]),
        None => Ok(vec![]),
    };
}

fn assert_kind<'a>(
    context: &ParsingContext,
    function: &str,
    node: &'a SyntaxNode,
    expected_kind: SyntaxKind,
    help: &str,
) -> Result<&'a SyntaxNode, Error> {
    if node.kind() == expected_kind {
        return Ok(node);
    }

    let pos = (
        usize::from(node.text_range().start()),
        usize::from(node.text_range().len()),
    );
    return Err(Error::UnexpectedArgument {
        function: function.to_string(),
        src: context.src(),
        argument_pos: pos.into(),
        // TODO: convert from SyntaxKind to friendlier names
        expected_type: format!("{:#?}", expected_kind),
        help: help.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use crate::deps::test_util;

    #[test]
    fn invalid_uptix_function() {
        let dependencies: Vec<_> = test_util::deps(
            r#"{
                uptixModule = uptix.nixosModules.uptix ./uptix.lock;
                version = uptix.version release;
            }"#,
        )
        .unwrap();
        assert_eq!(dependencies.len(), 0);
    }
}
