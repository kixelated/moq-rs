{ self, nixpkgs, flake-utils, ... }:
flake-utils.lib.eachDefaultSystem (system:
  let
    pkgs = nixpkgs.legacyPackages.${system};
  in
    with pkgs;
    {
      devShells.default = mkShell {
        nativeBuildInputs = [
          pkg-config
          libressl
          cargo
          ffmpeg
        ];
        LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
      };
    }
)
