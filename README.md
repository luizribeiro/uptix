# docknix

A tool for pinning (and updating) Docker images on NixOS configurations.

## Setup

On your `flake.nix`, just add this repository as an input and add the
`docknix.nixosModules.docknix` module to your configurations:

```nix
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/master";
  inputs.docknix.url = "github:luizribeiro/docknix";
  
  outputs = { nixpkgs, docknix, ... }: {
    nixosConfigurations.somehost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        # add this line to your configurations with the path where you will
        # want the docknix.lock file to be (a lot of times it will be in
        # the root of your project):
        (docknix.nixosModules.docknix ./docknix.lock)
        # ... your other modules go here as usual ...
      ];
    };
  };
}
```

Once that is done, you should be able to use it from your configurations.

If you have a shell setup on your flake, you will probably also want to
install `docknix` onto your `flake.nix`'s `devShell`:

```nix
{
  # ...
  devShell = pkgs.mkShell {
    buildInputs = [
      docknix.defaultPackage."${system}"
    ];
  };
  # ...
}
```

Alternatively, you can also just run `docknix` with:

```bash
$ nix run "github:luizribeiro/docknix"
```

## Usage

Once you have `docknix` setup, all you have to do is prefix your Docker image
names with `docknix.dockerImage` on your configurations. For example:

```nix
# note that docknix is now available as an argument on your configuration.
{ pkgs, docknix, ... }:

{
  virtualisation.oci-containers.containers = {
    homeassistant = {
      # this is all you need
      image = docknix.dockerImage "ghcr.io/home-assistant/home-assistant:stable";
      # ...
    };
  };
}
```

Once that is in place, run `docknix` from the command line and voilà:

```
$ docknix
Found 2 nix files
Parsing files... Done.
Found 1 docker image references
Looking for updates... Done.
Wrote docknix.lock successfully
```

Make sure to check the `docknix.lock` file into your source control
repository. This is the file that keeps track of which version of the
Docker image you are currently using.

Every time you run the `docknix` binary, it will find all of your
`docker.image` references and update the `docknix.lock` with the SHA256
digest for the latest version.

If you want to update your Docker images to their latest versions, simply
run `docknix` again.
