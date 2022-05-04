{
  description = "A tool for pinning external dependencies on Nix.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, ... }: {
    nixosModules.uptix = lockFile: {
      _module.args.uptix = import ./modules { inherit lockFile; };
    };
  } // utils.lib.eachSystem utils.lib.defaultSystems (system:
    let
      pkgs = import nixpkgs { inherit system; };
      exports = ''
        export OPENSSL_DIR="${pkgs.openssl.dev}"
        export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
      '';
    in
    with pkgs; {
      defaultPackage = self.packages."${system}".uptix;
      packages.uptix = rustPlatform.buildRustPackage {
        pname = "uptix";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        buildInputs = [
          openssl
          makeWrapper
        ];
        preBuild = exports;
        postInstall = ''
          wrapProgram $out/bin/uptix \
            --prefix PATH : ${lib.makeBinPath [ nix-prefetch-git ]}
        '';

        meta = {
          description = "A tool for pinning external dependencies on Nix.";
        };
      };

      devShell = mkShell {
        buildInputs = [
          openssl
        ];
        nativeBuildInputs = [
          # dependencies which go on the nix wrapper
          nix-prefetch-git
          # tools for development
          rustc
          cargo
          rust-analyzer
          rustfmt
        ];
        shellHook = exports;
        NIX_LD_LIBRARY_PATH = lib.makeLibraryPath [
          openssl.dev
        ];
      };
    }
  );
}
