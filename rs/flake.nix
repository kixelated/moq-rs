{
  description = "My reproducible Rust dev environment with naersk";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk.url = "github:nmattia/naersk";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      naersk,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rust = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
          extensions = [
            "rustfmt"
            "clippy"
          ];
        };

        naersk-lib = pkgs.callPackage naersk {
          rustc = rust;
          cargo = rust;
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
          moq-clock = naersk-lib.buildPackage {
            src = ./.;
            doCheck = false;
            CARGO_TARGET_DIR = "target";
            cargoBuildOptions = opts: opts ++ [ "--bin=moq-clock" ];
          };

          moq-relay = naersk-lib.buildPackage {
            src = ./.;
            doCheck = false;
            CARGO_TARGET_DIR = "target";
            cargoBuildOptions = opts: opts ++ [ "--bin=moq-relay" ];
          };

          hang = naersk-lib.buildPackage {
            src = ./.;
            doCheck = false;
            CARGO_TARGET_DIR = "target";
            cargoBuildOptions = opts: opts ++ [ "--bin=hang" ];
          };

          # Optional: expose all workspace bins at once
          default = naersk-lib.buildPackage {
            src = ./.;
            doCheck = false;
            CARGO_TARGET_DIR = "target";
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
