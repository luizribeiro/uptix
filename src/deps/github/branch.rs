use crate::deps::assert_kind;
use crate::deps::github;
use crate::deps::Lockable;
use crate::error::Error;
use crate::util;
use crate::util::ParsingContext;
use async_trait::async_trait;
use rnix::{SyntaxKind, SyntaxNode};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct GitHubBranch {
    owner: String,
    repo: String,
    branch: String,
    fetchSubmodules: Option<bool>,
    deepClone: Option<bool>,
    leaveDotGit: Option<bool>,
    override_scheme: Option<String>,
    override_domain: Option<String>,
    override_nix_sha256: Option<String>,
}

impl GitHubBranch {
    pub fn new(context: &ParsingContext, node: &SyntaxNode) -> Result<GitHubBranch, Error> {
        util::from_attr_set(assert_kind(
            context,
            "uptix.githubBranch",
            node,
            SyntaxKind::NODE_ATTR_SET,
            r#"here is an example of valid usage:

  uptix.githubBranch {
    owner = "luizribeiro";
    repo = "uptix";
    branch = "main";
  }"#,
        )?)
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
            "$GITHUB_BRANCH$:{}/{}:{}${}",
            self.owner,
            self.repo,
            self.branch,
            github::flags(self.fetchSubmodules, self.deepClone, self.leaveDotGit),
        );
    }

    async fn lock(&self) -> Result<Box<dyn erased_serde::Serialize>, Error> {
        let rev = fetch_github_branch_info(self).await?.commit.sha;
        let sha256 = match &self.override_nix_sha256 {
            Some(s) => s.to_string(),
            None => github::compute_nix_sha256(
                &self.owner,
                &self.repo,
                &rev,
                self.fetchSubmodules,
                self.deepClone,
                self.leaveDotGit,
            )?,
        };
        return Ok(Box::new(github::GitHubLock {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            rev,
            sha256,
            fetchSubmodules: self.fetchSubmodules.unwrap_or(false),
            deepClone: self.deepClone.unwrap_or(false),
            leaveDotGit: self.leaveDotGit.unwrap_or(false),
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::GitHubBranch;
    use crate::deps::test_util;
    use crate::deps::Lockable;
    use serde_json::json;

    #[test]
    fn it_parses() {
        let dependencies: Vec<_> = test_util::deps(
            r#"{
                uptix = fetchFromGitHub (uptix.githubBranch {
                    owner = "luizribeiro";
                    repo = "uptix";
                    branch = "main";
                });
                uptixWithOptions = fetchFromGitHub (uptix.githubBranch {
                    owner = "luizribeiro";
                    repo = "uptix";
                    branch = "main";
                    fetchSubmodules = true;
                });
            }"#,
        )
        .unwrap()
        .iter()
        .map(|d| d.as_git_hub_branch().unwrap().clone())
        .collect();
        let expected_dependencies = vec![
            GitHubBranch {
                owner: "luizribeiro".to_string(),
                repo: "uptix".to_string(),
                branch: "main".to_string(),
                ..Default::default()
            },
            GitHubBranch {
                owner: "luizribeiro".to_string(),
                repo: "uptix".to_string(),
                branch: "main".to_string(),
                fetchSubmodules: Some(true),
                ..Default::default()
            },
        ];
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
        assert_eq!(dependency.key(), "$GITHUB_BRANCH$:luizribeiro/uptix:main$");
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
            ..Default::default()
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
                "fetchSubmodules": false,
                "deepClone": false,
                "leaveDotGit": false,
            }),
        );

        mockito::reset();
    }

    #[test]
    fn it_provides_helpful_errors() {
        let result = test_util::deps("{ hass = uptix.githubBranch 42; }");
        assert!(result.is_err());
        match result {
            Err(crate::error::Error::UnexpectedArgument {
                function,
                src: _,
                argument_pos,
                expected_type,
                help: _,
            }) => {
                assert_eq!(function, "uptix.githubBranch");
                assert_eq!(expected_type, "NODE_ATTR_SET");
                assert_eq!(argument_pos, (28, 2).into());
            }
            _ => assert!(false),
        }
    }
}
