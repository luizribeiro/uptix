pub mod branch;
pub mod release;

use crate::error::Error;
use crate::util;
use serde::{Deserialize, Serialize};
use std::env;
use std::process::Command;

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct GitHubLock {
    owner: String,
    repo: String,
    rev: String,
    sha256: String,
    fetchSubmodules: bool,
    deepClone: bool,
    leaveDotGit: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct GitHubPrefetchInfo {
    sha256: String,
}

fn compute_nix_sha256(
    owner: &str,
    repo: &str,
    rev: &str,
    fetch_submodules: Option<bool>,
    deep_clone: Option<bool>,
    leave_dot_git: Option<bool>,
) -> Result<String, Error> {
    let mut options = vec![];
    if deep_clone.unwrap_or(false) {
        options.push("--deepClone");
    } else {
        options.push("--no-deepClone");
    }
    if fetch_submodules.unwrap_or(false) {
        options.push("--fetch-submodules");
    }
    if leave_dot_git.unwrap_or(false) || deep_clone.unwrap_or(false) {
        // deepClone implies on leaveDotGit also being enabled, see
        // https://nixos.org/manual/nixpkgs/stable/#fetchgit
        options.push("--leave-dotGit");
    }
    let output = Command::new("nix-prefetch-git")
        .args(options)
        .arg("--quiet")
        .arg("--rev")
        .arg(rev)
        .arg(format!("https://github.com/{}/{}/", owner, repo,))
        .output()
        .expect("failed to execute process");
    let prefetch_info: GitHubPrefetchInfo = serde_json::from_slice(&output.stdout)?;
    return Ok(prefetch_info.sha256);
}

pub fn flags(
    fetch_submodules: Option<bool>,
    deep_clone: Option<bool>,
    leave_dot_git: Option<bool>,
) -> String {
    return format!(
        "{}{}{}",
        if fetch_submodules.unwrap_or(false) {
            "f"
        } else {
            ""
        },
        if deep_clone.unwrap_or(false) { "d" } else { "" },
        if leave_dot_git.unwrap_or(false) {
            "l"
        } else {
            ""
        },
    );
}

pub async fn github_api_request(url: reqwest::Url) -> Result<String, Error> {
    let client = reqwest::Client::new();
    let mut request = client
        .request(reqwest::Method::GET, url)
        .header(reqwest::header::USER_AGENT, util::user_agent());

    // Add GitHub token if available
    if let Ok(token) = env::var("GITHUB_TOKEN") {
        request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {}", token));
    }

    let response = request.send().await?;
    
    // Check status before reading body
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        return Err(Error::StringError(format!(
            "GitHub API request failed with status {}: {}",
            status, body
        )));
    }
    
    Ok(response.text().await?)
}
