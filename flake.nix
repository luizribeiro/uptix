{
  description = "A tool for pinning external dependencies on Nix.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, ... }: {
    nixosModules.uptix = lockFile: { pkgs, ... }: {
      _module.args.uptix = import ./modules { inherit lockFile pkgs; };
    };
  } // utils.lib.eachSystem utils.lib.defaultSystems (system:
    let
      pkgs = import nixpkgs { inherit system; };
    in
    with pkgs; {
      defaultPackage = self.packages."${system}".uptix;
      packages.uptix = rustPlatform.buildRustPackage {
        pname = "uptix";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        cargoLock.outputHashes = {
          "dkregistry-0.5.1-alpha.0" = "sha256-s2F+g5e02HOLQGNPnl4NYe4IQqYI9R84by7hiT3J26I=";
        };
        buildInputs = [
          openssl
          makeWrapper
        ] ++ lib.optionals stdenv.isDarwin [
          libiconv
        ];
        preBuild = ''
          export OPENSSL_DIR="${pkgs.openssl.dev}"
          export OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib"
        '';
        postInstall = ''
          wrapProgram $out/bin/uptix \
            --prefix PATH : ${lib.makeBinPath [ nix-prefetch-git nix-prefetch-docker ]}
        '';

        meta = {
          description = "A tool for pinning external dependencies on Nix.";
        };
      };

      devShell = mkShell {
        buildInputs = [
          openssl
        ] ++ lib.optionals stdenv.isDarwin [
          libiconv
        ];
        nativeBuildInputs = [
          # dependencies which go on the nix wrapper
          nix-prefetch-git
          nix-prefetch-docker
          # tools for development
          rustc
          cargo
          rust-analyzer
          rustfmt
        ];
        OPENSSL_DIR = "${pkgs.openssl.dev}";
        OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
        NIX_LD_LIBRARY_PATH = lib.makeLibraryPath [
          openssl.dev
        ];
      };
    }
  );
}
