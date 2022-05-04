{ uptix, ... }:

{
  imports = [ ./hardware-configuration.nix ];

  virtualisation.oci-containers.containers = {
    homeassistant = {
      image = uptix.dockerImage "homeassistant/home-assistant:stable";
    };
  };
}
