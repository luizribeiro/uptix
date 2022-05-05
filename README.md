# uptix

[![CI](https://github.com/luizribeiro/uptix/actions/workflows/ci.yml/badge.svg)](https://github.com/luizribeiro/uptix/actions/workflows/ci.yml)

A tool for pinning (and updating) external dependencies on NixOS configurations.

## Setup

On your `flake.nix`, just add this repository as an input and add the
`uptix.nixosModules.uptix` module to your configurations:

```nix
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/master";
  inputs.uptix.url = "github:luizribeiro/uptix";
  
  outputs = { nixpkgs, uptix, ... }: {
    nixosConfigurations.somehost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        # add this line to your configurations with the path where you will
        # want the uptix.lock file to be (a lot of times it will be in
        # the root of your project):
        (uptix.nixosModules.uptix ./uptix.lock)
        # ... your other modules go here as usual ...
      ];
    };
  };
}
```

Once that is done, you should be able to use it from your configurations, as
`uptix` will be passed as an arugment to all modules on your configuration.

If you have a shell setup on your flake, you will probably also want to
install `uptix` onto your `flake.nix`'s `devShell`:

```nix
{
  # ...
  devShell = pkgs.mkShell {
    buildInputs = [
      uptix.defaultPackage."${system}"
    ];
  };
  # ...
}
```

Alternatively, you can also just run `uptix` with:

```bash
$ nix run "github:luizribeiro/uptix"
```

## Usage

Once you have `uptix` setup, all you have to do is use one of the functions
provided by the `uptix` module and call the `uptix` command from the command
line to populate the `uptix.`See the examples below for details on usage for
each type of dependency.

Make sure to check the `uptix.lock` file into your source control
repository. This is the file that keeps track of which version of the
Docker image you are currently using.

Every time you run the `uptix` binary, it will find all of your
`uptix` references and update the `uptix.lock` with the SHA256
digest for the latest version of each dependency.

### GitHub

For GitHub checkouts that are typically fetched with `fetchFromGitHub`, you
can use `uptix.githubBranch` as follows:

```nix
{ pkgs, uptix, stdenv, ... }:

stdenv.mkDerivation {
  pname = "foo";
  # ...
  src = pkgs.lib.fetchFromGitHub (uptix.githubBranch {
    owner = "torvalds";
    repo = "linux";
    branch = "master";
  });
  # ...
}
```

Note that this will use the latest commit on the `master` branch. Using
tags from GitHub releases is not yet supported.

### Docker

For Docker images, prefix the image names with `uptix.dockerImage` on your
configurations:

```nix
{ pkgs, uptix, ... }:

{
  virtualisation.oci-containers.containers = {
    homeassistant = {
      # this is all you need
      image = uptix.dockerImage "homeassistant/home-assistant:stable";
      # ...
    };
  };
}
```
