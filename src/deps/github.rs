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

#[derive(Serialize, Deserialize, Debug)]
struct GitHubCommitInfo {
    sha: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GitHubBranchInfo {
    commit: GitHubCommitInfo,
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
        let client = reqwest::Client::new();
        let url_as_str = format!(
            "https://api.github.com/repos/{}/{}/branches/{}",
            self.owner, self.repo, self.branch,
        );
        let url = reqwest::Url::parse(&url_as_str).unwrap();
        let response = client
            .request(reqwest::Method::GET, url)
            .header(reqwest::header::USER_AGENT, "uptix/0.1.0")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let branch_info: GitHubBranchInfo = serde_json::from_str(&response).unwrap();
        return Ok(Box::new(GitHubLock {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            rev: branch_info.commit.sha,
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
                "rev": "b28012d8b7f8ef54492c66f3a77074391e9818b9",
                "sha256": "foobar",
            }),
        );
    }
}
