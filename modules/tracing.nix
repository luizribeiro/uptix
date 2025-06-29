{ lockFile }:

with builtins;

let
  lockFor = key: (importJSON lockFile).${key};
  gitFlag = s: v: if v then s else "";
  gitFlags = { fetchSubmodules ? false, deepClone ? false, leaveDotGit ? false, ... }:
    concatStringsSep "" [
      (gitFlag "f" fetchSubmodules)
      (gitFlag "d" deepClone)
      (gitFlag "l" leaveDotGit)
    ];
  filterFalse = set: (listToAttrs (concatMap
    (name:
      if set.${name} == false
      then [ ]
      else [{ inherit name; value = set.${name}; }])
    (attrNames set))
  );
  importJSON = path: fromJSON (readFile path);
  hasPrefix = pref: str: substring 0 (stringLength pref) str == pref;
  
  # Tracing helper
  traceUptix = type: data: 
    builtins.trace "UPTIX_DEPENDENCY:${type}:${toJSON data}" data;
in
{
  dockerImage = name: 
    traceUptix "docker" { image = name; } 
    "${name}@${lockFor name}";
    
  githubBranch = { owner, repo, branch, ... } @ args:
    let result = (filterFalse (lockFor "$GITHUB_BRANCH$:${owner}/${repo}:${branch}\$${gitFlags args}"))
                // (removeAttrs args [ "branch" ]);
    in traceUptix "github-branch" { inherit owner repo branch; } result;
    
  githubRelease = { owner, repo, ... } @ args:
    let result = (filterFalse (lockFor "$GITHUB_RELEASE$:${owner}/${repo}\$${gitFlags args}"))
                // args;
    in traceUptix "github-release" { inherit owner repo; } result;
    
  version = githubRelease:
    let rev = githubRelease.rev; in
    if hasPrefix "v" rev
    then (substring 1 (stringLength rev) rev)
    else rev;
}