{ self, nixpkgs, flake-utils, fenix, ... }:
flake-utils.lib.eachDefaultSystem (system:
  let
    pkgs = nixpkgs.legacyPackages.${system};
  in
    {
      devShells = {
        default = with pkgs; mkShell {
          nativeBuildInputs = [
            pkg-config
            libressl
            cargo
            rustfmt
            ffmpeg
          ];
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
        };

        web =
          let
            rustToolchain = with fenix.packages.${system};
              combine [
                latest.rustc
                latest.cargo
                targets.wasm32-unknown-unknown.latest.rust-std
              ];
          in
            with pkgs;
            mkShell {
              nativeBuildInputs = [
                go
                nodejs_23
                biome
                rustToolchain
                wasm-pack
              ];
            };
      };
    }
)
