{ uptix, pkgs, ... }:

let
  # this is a rust package which is being built from the latest commit on
  # the main branch of the luizribeiro/hello-world-rs github repo
  helloWorldRS = pkgs.rustPlatform.buildRustPackage {
    pname = "hello-world-rs";
    version = "0.1.0";
    src = pkgs.fetchFromGitHub (uptix.githubBranch {
      owner = "luizribeiro";
      repo = "hello-world-rs";
      branch = "main";
    });
    cargoSha256 = "sha256-p6vLLM6A16o8dKLwUfP/qf4crnzlgp4f+Vs0ocRChE4=";
  };
  # this is the same rust package as above, but being built from the latest
  # released version on that same github repo
  releasedHelloWorldRS =
    let
      release = uptix.githubRelease {
        owner = "luizribeiro";
        repo = "hello-world-rs";
      };
    in
    pkgs.rustPlatform.buildRustPackage {
      pname = "released-hello-world-rs";
      version = uptix.version release;
      src = pkgs.fetchFromGitHub release;
      cargoSha256 = "sha256-QCh67x63vSgdDYg0I47hVqg+x4L2vU/shh3MJlO+sac=";
    };
in
{
  imports = [ ./hardware-configuration.nix ];

  virtualisation.oci-containers.containers = {
    # Example using pullDockerImage: image is stored in Nix store
    # Avoids runtime registry access and rate limits, but slower initial build
    homeassistant = {
      imageFile = uptix.pullDockerImage "homeassistant/home-assistant:stable";
      image = "homeassistant/home-assistant:stable";
    };

    # Example using dockerImage: image is pulled at runtime with pinned digest
    # Faster builds, but subject to registry rate limits at runtime
    zigbee2mqtt = {
      image = uptix.dockerImage "koenkk/zigbee2mqtt:latest";
    };

    # Another example using pullDockerImage
    postgres = {
      imageFile = uptix.pullDockerImage "postgres:15";
      image = "postgres:15";
      volumes = [ "postgres-data:/var/lib/postgresql/data" ];
      environment = {
        POSTGRES_PASSWORD = "postgres";
      };
    };
  };

  environment.systemPackages = [
    helloWorldRS
    releasedHelloWorldRS
  ];
}