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
    override_scheme: Option<String>,
    override_domain: Option<String>,
    override_nix_sha256: Option<String>,
}

impl GitHubRelease {
    pub fn new(node: &SyntaxNode) -> Result<GitHubRelease, &'static str> {
        match util::from_attr_set(node) {
            Ok(r) => Ok(r),
            _ => Err("Error while parsing arguments of uptix.githubRelease"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct GitHubLatestReleaseInfo {
    tag_name: String,
}

async fn fetch_github_latest_release(dependency: &GitHubRelease) -> GitHubLatestReleaseInfo {
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

#[async_trait]
impl Lockable for GitHubRelease {
    fn key(&self) -> String {
        return format!("$GITHUB_RELEASE$:{}/{}", self.owner, self.repo);
    }

    async fn lock(&self) -> Result<Box<dyn erased_serde::Serialize>, &'static str> {
        let rev = fetch_github_latest_release(self).await.tag_name;
        let sha256 = match &self.override_nix_sha256 {
            Some(s) => s.to_string(),
            None => github::compute_nix_sha256(&self.owner, &self.repo, &rev),
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
    use super::GitHubRelease;
    use crate::deps::collect_ast_dependencies;
    use crate::deps::Lockable;
    use serde_json::json;

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
            }),
        );

        mockito::reset();
    }
}
