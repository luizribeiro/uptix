use crate::deps::Lockable;
use crate::util;
use async_trait::async_trait;
use rnix::{SyntaxKind, SyntaxNode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Default, PartialEq, Clone, Debug)]
pub struct GitHub {
    owner: String,
    repo: String,
    branch: String,
    override_scheme: Option<String>,
    override_domain: Option<String>,
    override_nix_sha256: Option<String>,
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
            ..Default::default()
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

#[derive(Serialize, Deserialize, Debug)]
struct GitHubPrefetchInfo {
    sha256: String,
}

async fn fetch_github_branch_info(dependency: &GitHub) -> GitHubBranchInfo {
    let client = reqwest::Client::new();
    let url_as_str = format!(
        "{}://{}/repos/{}/{}/branches/{}",
        dependency
            .override_scheme
            .as_ref()
            .unwrap_or(&"https".to_string()),
        dependency
            .override_domain
            .as_ref()
            .unwrap_or(&"api.github.com".to_string()),
        dependency.owner,
        dependency.repo,
        dependency.branch,
    );
    let url = reqwest::Url::parse(&url_as_str).unwrap();
    let response = client
        .request(reqwest::Method::GET, url)
        .header(reqwest::header::USER_AGENT, util::user_agent())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    return serde_json::from_str(&response).unwrap();
}

fn compute_nix_sha256(dependency: &GitHub, rev: &str) -> String {
    if let Some(overridden_nix_sha256) = &dependency.override_nix_sha256 {
        return overridden_nix_sha256.to_string();
    }

    let output = Command::new("nix-prefetch-git")
        .arg("--quiet")
        .arg("--rev")
        .arg(rev)
        .arg(format!(
            "https://github.com/{}/{}/",
            dependency.owner, dependency.repo,
        ))
        .output()
        .expect("failed to execute process");
    let prefetch_info: GitHubPrefetchInfo = serde_json::from_slice(&output.stdout).unwrap();
    return prefetch_info.sha256;
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
        let rev = fetch_github_branch_info(self).await.commit.sha;
        let sha256 = compute_nix_sha256(self, &rev);
        return Ok(Box::new(GitHubLock {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            rev,
            sha256,
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
            ..Default::default()
        }];
        assert_eq!(dependencies, expected_dependencies);
    }

    #[test]
    fn it_has_a_key() {
        let dependency = GitHub {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
            branch: "main".to_string(),
            ..Default::default()
        };
        assert_eq!(dependency.key(), "$GITHUB_BRANCH$:luizribeiro/uptix:main");
    }

    #[tokio::test]
    async fn it_locks() {
        let address = mockito::server_address().to_string();
        let _branch_mock = mockito::mock("GET", "/repos/luizribeiro/uptix/branches/main")
            .match_header(
                &reqwest::header::USER_AGENT.to_string(),
                mockito::Matcher::Regex(r"^uptix/[0-9.]+$".to_string()),
            )
            .with_status(200)
            .with_body(
                r#"{
                    "commit": {
                        "sha": "b28012d8b7f8ef54492c66f3a77074391e9818b9"
                    }
                }"#,
            )
            .create();

        let dependency = GitHub {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
            branch: "main".to_string(),
            override_scheme: Some("http".to_string()),
            override_domain: Some(address),
            override_nix_sha256: Some(
                "1vxzg4wdjvfnc7fjqr9flza5y7gh69w0bpf7mhyf06ddcvq3p00j".to_string(),
            ),
        };
        let lock = dependency.lock().await.unwrap();
        let lock_value = serde_json::to_value(lock).unwrap();

        assert_eq!(
            lock_value,
            json!({
                "owner": "luizribeiro",
                "repo": "uptix",
                "rev": "b28012d8b7f8ef54492c66f3a77074391e9818b9",
                "sha256": "1vxzg4wdjvfnc7fjqr9flza5y7gh69w0bpf7mhyf06ddcvq3p00j",
            }),
        );

        mockito::reset();
    }
}
