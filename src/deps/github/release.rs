use crate::deps::github;
use crate::deps::Lockable;
use crate::util;
use async_trait::async_trait;
use rnix::SyntaxNode;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct GitHubRelease {
    owner: String,
    repo: String,
}

impl GitHubRelease {
    pub fn new(node: &SyntaxNode) -> Result<GitHubRelease, &'static str> {
        match util::from_attr_set(node) {
            Ok(r) => Ok(r),
            _ => Err("Error while parsing arguments of uptix.githubRelease"),
        }
    }
}

#[async_trait]
impl Lockable for GitHubRelease {
    fn key(&self) -> String {
        return format!(
            "$GITHUB_RELEASE$:{}/{}",
            self.owner, self.repo,
        );
    }

    async fn lock(&self) -> Result<Box<dyn erased_serde::Serialize>, &'static str> {
        return Ok(Box::new(github::GitHubLock {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            rev: "TODO".to_string(),
            sha256: "TODO".to_string(),
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::GitHubRelease;
    use crate::deps::collect_ast_dependencies;
    use crate::deps::Lockable;

    #[test]
    fn it_parses() {
        let ast = rnix::parse(
            r#"{
                uptix = fetchFromGitHub (uptix.githubRelease {
                    owner = "luizribeiro";
                    repo = "uptix";
                });
            }"#,
        );
        let dependencies: Vec<_> = collect_ast_dependencies(ast.node())
            .iter()
            .map(|d| d.as_git_hub_release().unwrap().clone())
            .collect();
        let expected_dependencies = vec![GitHubRelease {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
            ..Default::default()
        }];
        assert_eq!(dependencies, expected_dependencies);
    }

    #[test]
    fn it_has_a_key() {
        let dependency = GitHubRelease {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
            ..Default::default()
        };
        assert_eq!(dependency.key(), "$GITHUB_RELEASE$:luizribeiro/uptix");
    }

    #[tokio::test]
    async fn it_locks() {
    }
}
