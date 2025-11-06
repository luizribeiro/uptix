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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub friendly_version: Option<String>, // Human-readable version: "15.4-alpine", "2024-01-15", etc
    pub dep_type: String,
    pub description: String,
}

impl DependencyMetadata {
    /// Returns a friendly display string for the dependency type,
    /// including relevant selector information where appropriate.
    /// This is computed on-demand and not stored in the lock file.
    pub fn type_display(&self) -> String {
        let selector = self.selected_version.as_deref().unwrap_or("unknown");

        match self.dep_type.as_str() {
            "docker" => format!("docker-image ({})", selector),
            "github-branch" => format!("github-branch ({})", selector),
            "github-release" => {
                // For releases, "latest" is implied and doesn't add information
                if selector == "latest" {
                    "github-release".to_string()
                } else {
                    format!("github-release ({})", selector)
                }
            }
            _ => self.dep_type.clone(),
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
