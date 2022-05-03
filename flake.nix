{
  description = "A tool for pinning external dependencies on Nix.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, ... }: {
    nixosModules.uptix = lockFile: {
      _module.args.uptix = import ./lib.nix { inherit lockFile; };
    };
  } // utils.lib.eachSystem utils.lib.defaultSystems (system:
    let
      pkgs = import nixpkgs { inherit system; };
      exports = ''
        export OPENSSL_DIR="${pkgs.openssl.dev}"
        export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
      '';
    in
    {
      defaultPackage = self.packages."${system}".uptix;
      packages.uptix = pkgs.rustPlatform.buildRustPackage {
        pname = "uptix";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        buildInputs = [ pkgs.openssl ];
        preBuild = exports;
        meta = {
          description = "A tool for pinning external dependencies on Nix.";
        };
      };

      devShell = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          # tools for development
          rustc
          cargo
          rust-analyzer
          rustfmt
        ];
        shellHook = exports;
      };
    }
  );
}
