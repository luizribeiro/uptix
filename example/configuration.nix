{ uptix, pkgs, ... }:

let
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
in
{
  imports = [ ./hardware-configuration.nix ];

  virtualisation.oci-containers.containers = {
    homeassistant = {
      image = uptix.dockerImage "homeassistant/home-assistant:stable";
    };
  };

  environment.systemPackages = [
    helloWorldRS
  ];
}
