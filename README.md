# uptix

[![CI](https://github.com/luizribeiro/uptix/actions/workflows/ci.yml/badge.svg)](https://github.com/luizribeiro/uptix/actions/workflows/ci.yml)

A tool for pinning (and updating) external dependencies on NixOS configurations.

For now, only Docker images are supported.

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

Once that is done, you should be able to use it from your configurations.

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

Once you have `uptix` setup, all you have to do is prefix your Docker image
names with `uptix.dockerImage` on your configurations. For example:

```nix
# note that uptix is now available as an argument on your configuration.
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

Once that is in place, run `uptix` from the command line and voilà:

```
$ uptix
Found 2 nix files
Parsing files... Done.
Found 1 uptix dependencies
Looking for updates... Done.
Wrote uptix.lock successfully
```

Make sure to check the `uptix.lock` file into your source control
repository. This is the file that keeps track of which version of the
Docker image you are currently using.

Every time you run the `uptix` binary, it will find all of your
`uptix.dockerImage` references and update the `uptix.lock` with the SHA256
digest for the latest version.

If you want to update your Docker images to their latest versions, simply
run `uptix` again.
