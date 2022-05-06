{ lockFile }:

with builtins;

let
  lockFor = key: (importJSON lockFile).${key};
  gitFlag = s: v: if v then s else "";
  gitFlags = { fetchSubmodules ? false, deepClone ? false, leaveDotGit ? false }:
    concatStringsSep "" [
      (gitFlag "f" fetchSubmodules)
      (gitFlag "d" deepClone)
      (gitFlag "l" leaveDotGit)
    ];
  # from nixpkgs.lib
  importJSON = path: fromJSON (readFile path);
  hasPrefix = pref: str: substring 0 (stringLength pref) str == pref;
in
{
  dockerImage = name: "${name}@${lockFor name}";
  githubBranch = { owner, repo, branch, ... } @ args:
    (lockFor "$GITHUB_BRANCH$:${owner}/${repo}:${branch}\$${gitFlags args}")
    // (removeAttrs args [ "branch" ]);
  githubRelease = { owner, repo, ... } @ args:
    (lockFor "$GITHUB_RELEASE$:${owner}/${repo}\$${gitFlags args}")
    // args;
  version = githubRelease:
    let rev = githubRelease.rev; in
    if hasPrefix "v" rev
    then (substring 1 (stringLength rev) rev)
    else rev;
}
