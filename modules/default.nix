{ lockFile }:

let
  importJSON = path: builtins.fromJSON (builtins.readFile path);
  lockFor = key: (importJSON lockFile).${key};
in
{
  dockerImage = name: "${name}@${lockFor name}";
  githubBranch = { owner, repo, branch }:
    (lockFor "$GITHUB_BRANCH$:${owner}/${repo}:${branch}");
  githubRelease = { owner, repo }:
    (lockFor "$GITHUB_RELEASE$:${owner}/${repo}");
}
