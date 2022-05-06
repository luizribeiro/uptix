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
    homeassistant = {
      # this container is using the latest released docker image under the
      # stable tag
      image = uptix.dockerImage "homeassistant/home-assistant:stable";
    };
  };

  environment.systemPackages = [
    helloWorldRS
    releasedHelloWorldRS
  ];
}
