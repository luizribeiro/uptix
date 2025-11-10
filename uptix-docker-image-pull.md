# Feature Request: Add `dockerTools.pullImage` Support to Uptix

## Problem Statement

Currently, `uptix.dockerImage` only pins Docker images by digest but doesn't fetch them into the Nix store. This means images are still pulled at runtime by podman/docker, causing rate limiting issues.

### Current Behavior

```nix
# In uptix/modules/default.nix
dockerImage = name: "${name}@${lockDataFor name}";
```

This returns a string like:
```
koenkk/zigbee2mqtt:latest@sha256:3baac2a3b74a9945fbbcb665b3e6e0ace5df2d49b5502e3d26b66a5454316e81
```

### Issues with Current Approach

1. **Docker Hub Rate Limiting**: Each host pulls images from Docker Hub at runtime, hitting unauthenticated rate limits (100 pulls per 6 hours)
2. **Network Dependency**: Container startup requires internet access and working registry
3. **No Binary Cache Benefit**: Images aren't in the Nix store, so they can't be shared via binary cache
4. **Slower Deployments**: Each host must download the full image from Docker Hub
5. **Less Deterministic**: Runtime pull failures can cause deployment issues

### Example Error

```
Nov 09 14:21:49 krypton podman-zigbee2mqtt-start[41068]: time="2025-11-09T14:21:49-05:00" level=warning msg="Failed, retrying in 1s ... (1/3). Error: initializing source docker://koenkk/zigbee2mqtt@sha256:3baac2a3b74a9945fbbcb665b3e6e0ace5df2d49b5502e3d26b66a5454316e81: reading manifest sha256:3baac2a3b74a9945fbbcb665b3e6e0ace5df2d49b5502e3d26b66a5454316e81 in docker.io/koenkk/zigbee2mqtt: toomanyrequests: You have reached your unauthenticated pull rate limit. https://www.docker.com/increase-rate-limit"
```

## Proposed Solution

Change `uptix.dockerImage` to use `pkgs.dockerTools.pullImage` internally, so images are fetched at build time and stored in `/nix/store`.

### Proposed API

```nix
# In user's configuration.nix
virtualisation.oci-containers.containers.zigbee2mqtt = {
  imageFile = uptix.dockerImage "koenkk/zigbee2mqtt:latest";
  image = "koenkk/zigbee2mqtt:latest";
  # ... rest of config
};
```

### Implementation: Pre-compute Nix Hashes (Option A)

When `uptix update` runs, fetch each Docker image to compute both:
1. The image digest (already done)
2. The Nix store hash of the downloaded tarball (new)

Store both in `uptix.lock`:

```json
{
  "koenkk/zigbee2mqtt:latest": {
    "metadata": {
      "name": "koenkk/zigbee2mqtt",
      "selected_version": "latest",
      "resolved_version": "sha256:3baac2a3b74a9945fbbcb665b3e6e0ace5df2d49b5502e3d26b66a5454316e81",
      "dep_type": "docker",
      "description": "Docker image koenkk/zigbee2mqtt:latest from registry-1.docker.io"
    },
    "lock": {
      "imageDigest": "sha256:3baac2a3b74a9945fbbcb665b3e6e0ace5df2d49b5502e3d26b66a5454316e81",
      "sha256": "sha256-abc123..."  // Nix store hash of tarball
    }
  }
}
```

### Module Changes

The uptix NixOS module needs access to `pkgs` to call `dockerTools.pullImage`. Update the module setup:

```nix
# In uptix/flake.nix
nixosModules.uptix = lockFile: { pkgs, ... }: {
  _module.args.uptix = import ./modules { inherit lockFile pkgs; };
};
```

Then in `uptix/modules/default.nix`:

```nix
{ lockFile, pkgs }:

with builtins;

let
  lockFor = key: (importJSON lockFile).${key};
  lockDataFor = key:
    let entry = lockFor key;
    in if entry ? lock then entry.lock else entry;
  # ... other helpers ...
in
{
  dockerImage = name:
    let
      lockData = lockFor name;
      # Parse image reference (uptix already handles this for fetching)
      imageParts = parseImageRef name;
      imageDigest = lockData.lock.imageDigest;
      nixHash = lockData.lock.sha256;
    in
      pkgs.dockerTools.pullImage {
        imageName = imageParts.imageName;
        imageDigest = imageDigest;
        sha256 = nixHash;
        finalImageTag = imageParts.tag;
      };

  # Keep other functions (githubBranch, githubRelease, etc.)
  # ...
}
```

### Required Changes

1. **CLI (`uptix` command)**:
   - When updating Docker dependencies, fetch images to compute Nix hashes
   - This happens automatically during `uptix update`
   - Store both `imageDigest` and `sha256` in lock file

2. **Lock file format**:
   - Change Docker image lock format from string to object:
     ```json
     // Old format
     "lock": "sha256:3baac..."

     // New format
     "lock": {
       "imageDigest": "sha256:3baac...",
       "sha256": "sha256-abc..."
     }
     ```

3. **Nix module**:
   - Accept `pkgs` as a parameter in `nixosModules.uptix`
   - Pass `pkgs` to the modules function
   - Change `dockerImage` to return a derivation from `dockerTools.pullImage`
   - Reuse existing image reference parsing logic

## Benefits

1. **No Runtime Rate Limits**: Images pulled once during build, not per-host
2. **Binary Cache Support**: Images stored in `/nix/store` can be shared via binary cache
3. **Faster Deployments**: Hosts pull from local binary cache instead of Docker Hub
4. **More Reliable**: No dependency on external registries at deployment time
5. **Better for CI/CD**: Build once, deploy to many hosts
6. **Bandwidth Savings**: Large images downloaded once, not per host

## Migration

Since we're the only users of uptix, no backward compatibility needed. Just update the lock file format and change how `dockerImage` works.

Users will need to update their configs from:
```nix
# Old
virtualisation.oci-containers.containers.zigbee2mqtt = {
  image = uptix.dockerImage "koenkk/zigbee2mqtt:latest";
};
```

To:
```nix
# New
virtualisation.oci-containers.containers.zigbee2mqtt = {
  imageFile = uptix.dockerImage "koenkk/zigbee2mqtt:latest";
  image = "koenkk/zigbee2mqtt:latest";
};
```

## Implementation Steps

1. Update `uptix` CLI to compute Nix hashes when fetching Docker image digests
2. Update lock file format for Docker images to store both hashes
3. Update `nixosModules.uptix` in `flake.nix` to accept and pass `pkgs`
4. Update `dockerImage` function in `modules/default.nix` to call `dockerTools.pullImage`
5. Update all configs in ops repo to use new `imageFile` pattern
6. Run `uptix update` to regenerate lock file with Nix hashes

## References

- NixOS Manual: [`pkgs.dockerTools.pullImage`](https://nixos.org/manual/nixpkgs/stable/#ssec-pkgs-dockerTools-pullImage)
- NixOS Option: [`virtualisation.oci-containers.containers.<name>.imageFile`](https://search.nixos.org/options?show=virtualisation.oci-containers.containers.%3Cname%3E.imageFile)
- Docker Hub Rate Limiting: https://www.docker.com/increase-rate-limit
