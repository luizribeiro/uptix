use crate::deps::assert_kind;
use crate::deps::github;
use crate::deps::{DependencyMetadata, LockEntry, Lockable};
use crate::error::Error;
use crate::util;
use crate::util::ParsingContext;
use async_trait::async_trait;
use rnix::{SyntaxKind, SyntaxNode};
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
    pub fn new(context: &ParsingContext, node: &SyntaxNode) -> Result<GitHubRelease, Error> {
        util::from_attr_set(assert_kind(
            context,
            "uptix.githubRelease",
            node,
            SyntaxKind::NODE_ATTR_SET,
            r#"here is an example of valid usage:

  uptix.githubRelease {
    owner = "luizribeiro";
    repo = "uptix";
  }"#,
        )?)
    }

    /// Reconstructs a GitHubRelease dependency from a lock entry.
    pub fn from_lock_entry(entry: &LockEntry) -> Option<GitHubRelease> {
        let lock_data = serde_json::from_value::<github::GitHubLock>(entry.lock.clone()).ok()?;

        Some(GitHubRelease {
            owner: lock_data.owner,
            repo: lock_data.repo,
            fetchSubmodules: Some(lock_data.fetchSubmodules),
            deepClone: Some(lock_data.deepClone),
            leaveDotGit: Some(lock_data.leaveDotGit),
            override_scheme: None,
            override_domain: None,
            override_nix_sha256: None,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct GitHubLatestReleaseInfo {
    tag_name: String,
}

async fn fetch_github_latest_release(
    dependency: &GitHubRelease,
) -> Result<GitHubLatestReleaseInfo, Error> {
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
    let response = github::github_api_request(url).await?;
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

    fn matches(&self, pattern: &str) -> bool {
        // Match the internal format
        if pattern == self.key() {
            return true;
        }

        // Match owner/repo format (without branch, so it's a release)
        if !pattern.contains(':') && pattern.contains('/') {
            let pattern_parts: Vec<&str> = pattern.split('/').collect();
            if pattern_parts.len() == 2 {
                return pattern_parts[0] == self.owner && pattern_parts[1] == self.repo;
            }
        }

        false
    }

    async fn lock_with_metadata(&self) -> Result<LockEntry, Error> {
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
        let lock_data = github::GitHubLock {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            rev: rev.clone(),
            sha256,
            fetchSubmodules: self.fetchSubmodules.unwrap_or(false),
            deepClone: self.deepClone.unwrap_or(false),
            leaveDotGit: self.leaveDotGit.unwrap_or(false),
        };

        let metadata = DependencyMetadata {
            name: format!("{}/{}", self.owner, self.repo),
            selected_version: Some("latest".to_string()),
            resolved_version: Some(rev),
            dep_type: "github-release".to_string(),
            description: format!("GitHub release from {}/{}", self.owner, self.repo),
        };

        Ok(LockEntry {
            metadata,
            lock: serde_json::to_value(lock_data)?,
        })
    }

    fn type_display(&self) -> String {
        // For releases, "latest" is implied and doesn't add information
        "github-release".to_string()
    }

    fn friendly_version(&self, resolved_version: &str) -> String {
        // GitHub release tags are already human-friendly (e.g., "v0.1.0")
        resolved_version.to_string()
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
        let lock_entry = dependency.lock_with_metadata().await.unwrap();
        let lock_value = lock_entry.lock;

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

    #[tokio::test]
    #[serial_test::serial]
    async fn it_uses_github_token_when_set() {
        use std::env;

        // Set GITHUB_TOKEN for this test
        env::set_var("GITHUB_TOKEN", "test-token-release");

        let address = mockito::server_address().to_string();
        let _latest_release_mock = mockito::mock("GET", "/repos/luizribeiro/uptix/releases/latest")
            .match_header(
                &reqwest::header::USER_AGENT.to_string(),
                mockito::Matcher::Regex(r"^uptix/[0-9.]+$".to_string()),
            )
            .match_header(
                &reqwest::header::AUTHORIZATION.to_string(),
                "Bearer test-token-release",
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

        let result = dependency.lock_with_metadata().await;

        // Clean up first to ensure it happens even on panic
        env::remove_var("GITHUB_TOKEN");
        mockito::reset();

        // Now check the result
        let lock_entry = result.unwrap();
        let lock_value = lock_entry.lock;

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
    }

    #[test]
    fn test_github_release_matches() {
        let release = GitHubRelease {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
            ..Default::default()
        };

        // Should match the internal key format
        assert!(release.matches("$GITHUB_RELEASE$:luizribeiro/uptix$"));

        // Should match owner/repo format (for releases)
        assert!(release.matches("luizribeiro/uptix"));

        // Should not match with branch format
        assert!(!release.matches("luizribeiro/uptix:main"));

        // Should not match different repos
        assert!(!release.matches("other/repo"));
        assert!(!release.matches("luizribeiro/other"));

        // Should not match partial names
        assert!(!release.matches("luizribeiro"));
        assert!(!release.matches("uptix"));
    }

    #[test]
    fn test_github_release_with_flags_matches() {
        let release = GitHubRelease {
            owner: "luizribeiro".to_string(),
            repo: "uptix".to_string(),
            fetchSubmodules: Some(true),
            ..Default::default()
        };

        // Should match the internal key format with flags
        assert!(release.matches("$GITHUB_RELEASE$:luizribeiro/uptix$f"));

        // Should still match the simple format
        assert!(release.matches("luizribeiro/uptix"));
    }

    #[test]
    fn it_provides_helpful_errors() {
        let result = test_util::deps("{ hass = uptix.githubRelease 42; }");
        assert!(result.is_err());
        match result {
            Err(crate::error::Error::UnexpectedArgument {
                function,
                src: _,
                argument_pos,
                expected_type,
                help: _,
            }) => {
                assert_eq!(function, "uptix.githubRelease");
                assert_eq!(expected_type, "NODE_ATTR_SET");
                assert_eq!(argument_pos, (29, 2).into());
            }
            _ => assert!(false),
        }
    }
}
