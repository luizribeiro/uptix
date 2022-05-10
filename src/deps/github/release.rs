use crate::deps::github;
use crate::deps::Lockable;
use crate::error::Error;
use crate::util;
use async_trait::async_trait;
use rnix::SyntaxNode;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, PartialEq, Clone, Debug)]
#[allow(non_snake_case)]
pub struct GitHubRelease {
    owner: String,
    repo: String,
    fetchSubmodules: Option<bool>,
    deepClone: Option<bool>,
    leaveDotGit: Option<bool>,
    override_scheme: Option<String>,
    override_domain: Option<String>,
    override_nix_sha256: Option<String>,
}

impl GitHubRelease {
    pub fn new(node: &SyntaxNode) -> Result<GitHubRelease, Error> {
        util::from_attr_set(node)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct GitHubLatestReleaseInfo {
    tag_name: String,
}

async fn fetch_github_latest_release(
    dependency: &GitHubRelease,
) -> Result<GitHubLatestReleaseInfo, Error> {
    let client = reqwest::Client::new();
    let url_as_str = format!(
        "{}://{}/repos/{}/{}/releases/latest",
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
    );
    let url = reqwest::Url::parse(&url_as_str)?;
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
impl Lockable for GitHubRelease {
    fn key(&self) -> String {
        return format!(
            "$GITHUB_RELEASE$:{}/{}${}",
            self.owner,
            self.repo,
            github::flags(self.fetchSubmodules, self.deepClone, self.leaveDotGit)
        );
    }

    async fn lock(&self) -> Result<Box<dyn erased_serde::Serialize>, Error> {
        let rev = fetch_github_latest_release(self).await?.tag_name;
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
    use super::GitHubRelease;
    use crate::deps::test_util;
    use crate::deps::Lockable;
    use serde_json::json;

    #[test]
    fn it_parses() {
        let dependencies: Vec<_> = test_util::deps(
            r#"{
                uptix = fetchFromGitHub (uptix.githubRelease {
                    owner = "luizribeiro";
                    repo = "uptix";
                });
            }"#,
        )
        .unwrap()
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
        assert_eq!(dependency.key(), "$GITHUB_RELEASE$:luizribeiro/uptix$");
    }

    #[tokio::test]
    async fn it_locks() {
        let address = mockito::server_address().to_string();
        let _latest_release_mock = mockito::mock("GET", "/repos/luizribeiro/uptix/releases/latest")
            .match_header(
                &reqwest::header::USER_AGENT.to_string(),
                mockito::Matcher::Regex(r"^uptix/[0-9.]+$".to_string()),
            )
            .with_status(200)
            .with_body(
                r#"{
                    "tag_name": "v0.1.0"
                }"#,
            )
            .create();

        let dependency = GitHubRelease {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
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
                "rev": "v0.1.0",
                "sha256": "1vxzg4wdjvfnc7fjqr9flza5y7gh69w0bpf7mhyf06ddcvq3p00j",
                "fetchSubmodules": false,
                "deepClone": false,
                "leaveDotGit": false,
            }),
        );

        mockito::reset();
    }
}
