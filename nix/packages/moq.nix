{ self, nixpkgs, flake-utils, ... }:
flake-utils.lib.eachDefaultSystem (system:
  let
    pkgs = import nixpkgs {
      inherit system;
    };
  in
    with pkgs;
    {
      packages = rec {
        moq-relay = rustPlatform.buildRustPackage rec {
          pname = "moq-relay";
          version = "0.6.7";

          src = ../../.;

          cargoHash = "sha256-kOAulF1OqR1VBKSy15RURJEk2ZpgZIFxPwrr03RbvPk=";

          nativeBuildInputs = [ pkg-config ];

          buildInputs = [ libressl ];

          meta = {
            description = "Media Over QUIC relay server";
            mainProgram = "moq-relay";
            homepage = "https://quic.video/";
            changelog = "https://github.com/kixelated/moq-rs/releases/tag/moq-relay-v${version}";
          };
        };
      };
    }
)
