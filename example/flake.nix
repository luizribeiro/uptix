{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/master";
  inputs.uptix.url = "github:luizribeiro/uptix";

  outputs = { nixpkgs, uptix, ... }:
    let
      system = "x86_64-linux";
    in
    {
      nixosConfigurations.somehost = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          (uptix.nixosModules.uptix ./uptix.lock)
          ./configuration.nix
        ];
      };

      devShell.${system} =
        let
          pkgs = (import nixpkgs { inherit system; });
        in
        pkgs.mkShell {
          buildInputs = [
            uptix.defaultPackage.x86_64-linux
          ];
        };
    };
}
