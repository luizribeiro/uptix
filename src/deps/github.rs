use crate::deps::Lockable;
use async_trait::async_trait;
use rnix::{SyntaxKind, SyntaxNode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(PartialEq, Clone, Debug)]
pub struct GitHub {
    owner: String,
    repo: String,
    branch: String,
}

impl GitHub {
    pub fn new(node: &SyntaxNode) -> Result<GitHub, &'static str> {
        if node.kind() != SyntaxKind::NODE_ATTR_SET {
            return Err("Unexpected node");
        }

        let mut args: HashMap<String, String> = HashMap::new();
        for child in node.children() {
            if child.kind() != SyntaxKind::NODE_KEY_VALUE {
                return Err("Unexpected node");
            }
            let (key, value) = extract_key_value(&child);
            args.insert(key, value);
        }

        return Ok(GitHub {
            owner: args.get("owner").unwrap().to_string(),
            repo: args.get("repo").unwrap().to_string(),
            branch: args.get("branch").unwrap().to_string(),
        });
    }
}

fn extract_key_value(node: &SyntaxNode) -> (String, String) {
    let key = node.first_child().unwrap();
    let mut value = key.next_sibling().unwrap().text().to_string();
    value.pop();
    value.remove(0);
    return (key.text().to_string(), value);
}

#[derive(Serialize, Deserialize)]
struct GitHubLock {
    owner: String,
    repo: String,
    rev: String,
    sha256: String,
}

#[async_trait]
impl Lockable for GitHub {
    fn key(&self) -> String {
        return format!(
            "$GITHUB_BRANCH$:{}/{}:{}",
            self.owner, self.repo, self.branch,
        );
    }

    async fn lock(&self) -> Result<Box<dyn erased_serde::Serialize>, &'static str> {
        return Ok(Box::new(GitHubLock {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            // TODO: replace with commit hash
            rev: self.branch.clone(),
            // TODO: replace with nix hash for commit
            sha256: "foobar".to_string(),
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::GitHub;
    use crate::deps::collect_ast_dependencies;
    use crate::deps::Lockable;
    use serde_json::json;

    #[test]
    fn it_parses() {
        let ast = rnix::parse(
            r#"{
                uptix = fetchFromGitHub (uptix.github {
                    owner = "luizribeiro";
                    repo = "uptix";
                    branch = "main";
                });
            }"#,
        );
        let dependencies: Vec<_> = collect_ast_dependencies(ast.node())
            .iter()
            .map(|d| d.as_git_hub().unwrap().clone())
            .collect();
        let expected_dependencies = vec![GitHub {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
            branch: "main".to_string(),
        }];
        assert_eq!(dependencies, expected_dependencies);
    }

    #[test]
    fn it_has_a_key() {
        let dependency = GitHub {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
            branch: "main".to_string(),
        };
        assert_eq!(dependency.key(), "$GITHUB_BRANCH$:luizribeiro/uptix:main");
    }

    #[tokio::test]
    async fn it_locks() {
        let dependency = GitHub {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
            branch: "main".to_string(),
        };

        let lock = dependency.lock().await.unwrap();
        let lock_value = serde_json::to_value(lock).unwrap();
        assert_eq!(
            lock_value,
            json!({
                "owner": "luizribeiro",
                "repo": "uptix",
                "rev": "main",
                "sha256": "foobar",
            }),
        );
    }
}
