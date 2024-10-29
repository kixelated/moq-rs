{ self, nixpkgs, flake-utils, ... }:
flake-utils.lib.eachDefaultSystem (system:
  let
    pkgs = nixpkgs.legacyPackages.${system};
    moq-relay-version = (pkgs.lib.importTOML ../../moq-relay/Cargo.toml).package.version;
  in
    with pkgs;
    {
      packages = rec {
        moq-relay = rustPlatform.buildRustPackage rec {
          pname = "moq-relay";
          version = moq-relay-version;

          src = ../../.;

          cargoLock = {
            lockFile = ../../Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          nativeBuildInputs = [ pkg-config ];

          buildInputs = [ libressl ]
                        ++ lib.optionals stdenv.isDarwin [
                          darwin.apple_sdk.frameworks.Security
                          darwin.apple_sdk.frameworks.SystemConfiguration
                        ];
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
