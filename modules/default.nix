{ lockFile }:

with builtins;

let
  lockFor = key: (importJSON lockFile).${key};
  # from nixpkgs.lib
  importJSON = path: fromJSON (readFile path);
  hasPrefix = pref: str: substring 0 (stringLength pref) str == pref;
in
{
  dockerImage = name: "${name}@${lockFor name}";
  githubBranch = { owner, repo, branch }:
    (lockFor "$GITHUB_BRANCH$:${owner}/${repo}:${branch}");
  githubRelease = { owner, repo }:
    (lockFor "$GITHUB_RELEASE$:${owner}/${repo}");
  version = githubRelease:
    let rev = githubRelease.rev; in
    if hasPrefix "v" rev
    then (substring 1 (stringLength rev) rev)
    else rev;
}
