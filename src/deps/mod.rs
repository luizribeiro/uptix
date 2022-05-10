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
use erased_serde::Serialize;
use rnix::{SyntaxKind, SyntaxNode};
use std::fs;

#[derive(EnumAsInner, Clone, Debug)]
pub enum Dependency {
    Docker(Docker),
    GitHubBranch(GitHubBranch),
    GitHubRelease(GitHubRelease),
}

#[async_trait]
pub trait Lockable {
    fn key(&self) -> String;
    async fn lock(&self) -> Result<Box<dyn Serialize>, Error>;
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
            "uptix.githubRelease" => {
                Ok(Some(Dependency::GitHubRelease(GitHubRelease::new(&node)?)))
            }
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

    pub async fn lock(&self) -> Result<Box<dyn Serialize>, Error> {
        match self {
            Dependency::Docker(d) => d.lock().await,
            Dependency::GitHubBranch(d) => d.lock().await,
            Dependency::GitHubRelease(d) => d.lock().await,
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
