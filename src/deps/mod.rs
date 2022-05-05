mod docker;
mod github;

use crate::deps::docker::Docker;
use crate::deps::github::branch::GitHubBranch;
use crate::deps::github::release::GitHubRelease;
use async_trait::async_trait;
use enum_as_inner::EnumAsInner;
use erased_serde::Serialize;
use rnix::{SyntaxKind, SyntaxNode};
use std::fs;

#[derive(EnumAsInner, Debug)]
pub enum Dependency {
    Docker(Docker),
    GitHubBranch(GitHubBranch),
    GitHubRelease(GitHubRelease),
}

#[async_trait]
pub trait Lockable {
    fn key(&self) -> String;
    async fn lock(&self) -> Result<Box<dyn Serialize>, &'static str>;
}

impl Dependency {
    pub fn new(func: &str, node: &SyntaxNode) -> Result<Dependency, &'static str> {
        let dep = match func {
            "uptix.dockerImage" => Dependency::Docker(Docker::new(&node)?),
            "uptix.githubBranch" => Dependency::GitHubBranch(GitHubBranch::new(&node)?),
            "uptix.githubRelease" => Dependency::GitHubRelease(GitHubRelease::new(&node)?),
            _ => return Err("Unknown uptix function"),
        };
        return Ok(dep);
    }

    pub fn key(&self) -> String {
        match self {
            Dependency::Docker(d) => d.key(),
            Dependency::GitHubBranch(d) => d.key(),
            Dependency::GitHubRelease(d) => d.key(),
        }
    }

    pub async fn lock(&self) -> Result<Box<dyn Serialize>, &'static str> {
        match self {
            Dependency::Docker(d) => d.lock().await,
            Dependency::GitHubBranch(d) => d.lock().await,
            Dependency::GitHubRelease(d) => d.lock().await,
        }
    }
}

pub fn collect_file_dependencies(file_path: &str) -> Vec<Dependency> {
    let content = fs::read_to_string(file_path).unwrap();
    let ast = rnix::parse(&content);
    return collect_ast_dependencies(ast.node());
}

fn collect_ast_dependencies(node: SyntaxNode) -> Vec<Dependency> {
    if node.kind() != SyntaxKind::NODE_SELECT {
        return node
            .children()
            .map(|c| collect_ast_dependencies(c))
            .flatten()
            .collect();
    }

    let func = node.text().to_string();
    if !func.starts_with("uptix.") {
        return vec![];
    }

    let value_node = node.next_sibling();
    if value_node.is_none() {
        return vec![];
    }

    let dependency = match <Dependency>::new(&func, &value_node.unwrap()) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    return vec![dependency];
}

#[cfg(test)]
mod tests {
    use super::collect_ast_dependencies;

    #[test]
    fn invalid_uptix_function() {
        let ast = rnix::parse(
            r#"{
                uptixModule = uptix.nixosModules.uptix;
            }"#,
        );
        let dependencies: Vec<_> = collect_ast_dependencies(ast.node());
        assert_eq!(dependencies.len(), 0);
    }
}
