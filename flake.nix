{
  description = "A tool for pinning Docker dependencies on Nix.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, ... }: {
    overlay = final: prev:
      let system = final.system; in
      {
        docknix = {
          docknix = final.rustPlatform.buildRustPackage {
            pname = "docknix";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            buildInputs = [ prev.openssl ];
            preBuild = ''
              export OPENSSL_DIR=${prev.openssl.dev}
              export OPENSSL_LIB_DIR=${prev.openssl.out}/lib
            '';
            meta = {
              description = "A tool for pinning Docker dependencies on Nix.";
            };
          };
        };
      };

  } // utils.lib.eachSystem utils.lib.defaultSystems (system:
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ self.overlay ];
      };
    in
    {
      defaultPackage = self.packages."${system}".docknix;
      packages.docknix = pkgs.docknix.docknix;
    }
  );
}
