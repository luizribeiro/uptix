use crate::deps::github;
use crate::deps::Lockable;
use crate::error::Error;
use crate::util;
use async_trait::async_trait;
use rnix::SyntaxNode;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct GitHubBranch {
    owner: String,
    repo: String,
    branch: String,
    override_scheme: Option<String>,
    override_domain: Option<String>,
    override_nix_sha256: Option<String>,
}

impl GitHubBranch {
    pub fn new(node: &SyntaxNode) -> Result<GitHubBranch, Error> {
        util::from_attr_set(node)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct GitHubCommitInfo {
    sha: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GitHubBranchInfo {
    commit: GitHubCommitInfo,
}

async fn fetch_github_branch_info(dependency: &GitHubBranch) -> Result<GitHubBranchInfo, Error> {
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
        .await?
        .text()
        .await?;
    return Ok(serde_json::from_str(&response)?);
}

#[async_trait]
impl Lockable for GitHubBranch {
    fn key(&self) -> String {
        return format!(
            "$GITHUB_BRANCH$:{}/{}:{}",
            self.owner, self.repo, self.branch,
        );
    }

    async fn lock(&self) -> Result<Box<dyn erased_serde::Serialize>, Error> {
        let rev = fetch_github_branch_info(self).await?.commit.sha;
        let sha256 = match &self.override_nix_sha256 {
            Some(s) => s.to_string(),
            None => github::compute_nix_sha256(&self.owner, &self.repo, &rev)?,
        };
        return Ok(Box::new(github::GitHubLock {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            rev,
            sha256,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::GitHubBranch;
    use crate::deps::collect_ast_dependencies;
    use crate::deps::Lockable;
    use serde_json::json;

    #[test]
    fn it_parses() {
        let ast = rnix::parse(
            r#"{
                uptix = fetchFromGitHub (uptix.githubBranch {
                    owner = "luizribeiro";
                    repo = "uptix";
                    branch = "main";
                });
            }"#,
        );
        let dependencies: Vec<_> = collect_ast_dependencies(ast.node())
            .unwrap()
            .iter()
            .map(|d| d.as_git_hub_branch().unwrap().clone())
            .collect();
        let expected_dependencies = vec![GitHubBranch {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
            branch: "main".to_string(),
            ..Default::default()
        }];
        assert_eq!(dependencies, expected_dependencies);
    }

    #[test]
    fn it_has_a_key() {
        let dependency = GitHubBranch {
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

        let dependency = GitHubBranch {
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
