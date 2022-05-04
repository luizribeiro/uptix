{ modulesPath, ... }:

{
  boot.loader.grub.device = "nodev";
  fileSystems."/boot" = {
    device = "/dev/disk/by-label/boot";
    fsType = "vfat";
  };
  fileSystems."/" = {
    device = "/dev/disk/by-label/root";
    fsType = "ext4";
  };
}
