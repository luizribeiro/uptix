{
  inputs = {
    # For this example, we're using the local uptix source from the parent directory
    # In a real project, you would use the github URL instead:
    # uptix.url = "github:luizribeiro/uptix";
    uptix.url = "path:..";
  };

  outputs = { uptix, ... }:
    let
      system = "x86_64-linux";
      # Use uptix's nixpkgs to ensure compatibility
      nixpkgs = uptix.inputs.nixpkgs;
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