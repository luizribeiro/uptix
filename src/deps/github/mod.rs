pub mod branch;
pub mod release;

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize)]
pub struct GitHubLock {
    owner: String,
    repo: String,
    rev: String,
    sha256: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GitHubPrefetchInfo {
    sha256: String,
}

fn compute_nix_sha256(owner: &str, repo: &str, rev: &str) -> String {
    let output = Command::new("nix-prefetch-git")
        .arg("--quiet")
        .arg("--rev")
        .arg(rev)
        .arg(format!("https://github.com/{}/{}/", owner, repo,))
        .output()
        .expect("failed to execute process");
    let prefetch_info: GitHubPrefetchInfo = serde_json::from_slice(&output.stdout).unwrap();
    return prefetch_info.sha256;
}
