{ lockFile, pkgs }:

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
  # Parse a Docker image reference into its components
  parseImageRef = imageRef:
    let
      # Split on @ to separate name from digest (if present)
      parts = match "([^@]+)(@.+)?" imageRef;
      nameAndTag = elemAt parts 0;
      # Split name:tag
      tagParts = match "([^:]+):?(.+)?" nameAndTag;
      imageName = elemAt tagParts 0;
      tag = let t = elemAt tagParts 1; in if t == null then "latest" else t;
    in {
      inherit imageName tag;
    };
in
{
  # Returns image reference with digest pinned for runtime pulls
  # Faster deployments, but subject to registry rate limits
  # Usage: image = uptix.dockerImage "postgres:15";
  dockerImage = name:
    let
      lockData = lockDataFor name;
      # Handle both old (string) and new (object) lock formats
      imageDigest = if isString lockData then lockData else lockData.imageDigest;
    in
      "${name}@${imageDigest}";

  # Returns a derivation that pulls the image into the Nix store
  # Slower deployments, but avoids runtime registry access and rate limits
  # Usage: imageFile = uptix.pullDockerImage "postgres:15"; image = "postgres:15";
  pullDockerImage = name:
    let
      lockData = lockDataFor name;
      imageParts = parseImageRef name;
      # The lock data is now an object with imageDigest and sha256
      imageDigest = lockData.imageDigest;
      sha256 = lockData.sha256;
    in
      pkgs.dockerTools.pullImage {
        imageName = imageParts.imageName;
        imageDigest = imageDigest;
        sha256 = sha256;
        finalImageTag = imageParts.tag;
      };
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
