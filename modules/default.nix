{ lockFile }:

with builtins;

let
  lockFor = key: (importJSON lockFile).${key};
  lockDataFor = key: 
    let entry = lockFor key;
    in if entry ? lock then entry.lock else entry;
  gitFlag = s: v: if v then s else "";
  gitFlags = { fetchSubmodules ? false, deepClone ? false, leaveDotGit ? false, ... }:
    concatStringsSep "" [
      (gitFlag "f" fetchSubmodules)
      (gitFlag "d" deepClone)
      (gitFlag "l" leaveDotGit)
    ];
  # for some reason fetchFromGithub uses fetchZip if all of the flags are false, so we
  # filter any flags that are set to false
  filterFalse = set: (listToAttrs (concatMap
    (name:
      if set.${name} == false
      then [ ]
      else [{ inherit name; value = set.${name}; }])
    (attrNames set))
  );
  # from nixpkgs.lib
  importJSON = path: fromJSON (readFile path);
  hasPrefix = pref: str: substring 0 (stringLength pref) str == pref;
in
{
  dockerImage = name: "${name}@${lockDataFor name}";
  githubBranch = { owner, repo, branch, ... } @ args:
    (filterFalse (lockDataFor "$GITHUB_BRANCH$:${owner}/${repo}:${branch}\$${gitFlags args}"))
    // (removeAttrs args [ "branch" ]);
  githubRelease = { owner, repo, ... } @ args:
    (filterFalse (lockDataFor "$GITHUB_RELEASE$:${owner}/${repo}\$${gitFlags args}"))
    // args;
  version = githubRelease:
    let rev = githubRelease.rev; in
    if hasPrefix "v" rev
    then (substring 1 (stringLength rev) rev)
    else rev;
}
