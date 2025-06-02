{
  description = "My reproducible Rust dev environment with naersk";

  inputs = {
    fenix.url = "github:nix-community/fenix";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nmattia/naersk";
  };

  outputs =
    {
      self,
      fenix,
      nixpkgs,
      flake-utils,
      naersk,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        rust =
          with fenix.packages.${system};
          combine [
            stable.rustc
            stable.cargo
            stable.clippy
            stable.rustfmt
            targets.wasm32-unknown-unknown.latest.rust-std
          ];

        naersk' = naersk.lib.${system}.override {
          cargo = rust;
          rustc = rust;
        };

        gst-deps = with pkgs.gst_all_1; [
          gstreamer
          gst-plugins-base
          gst-plugins-good
          gst-plugins-bad
          gst-plugins-ugly
          gst-libav
        ];

        common-deps = [
          rust
          pkgs.just
        ] ++ gst-deps;

      in
      {
        packages = {
          moq-clock = naersk'.buildPackage {
            pname = "moq-clock";
            src = ./.;
          };

          moq-relay = naersk'.buildPackage {
            pname = "moq-relay";
            src = ./.;
          };

          hang = naersk'.buildPackage {
            pname = "hang";
            src = ./.;
          };

          default = naersk'.buildPackage {
            src = ./.;
          };
        };

        devShell = pkgs.mkShell {
          packages = common-deps ++ [
            pkgs.cargo-sort
            pkgs.cargo-shear
            pkgs.cargo-audit
          ];
        };
      }
    );
}
