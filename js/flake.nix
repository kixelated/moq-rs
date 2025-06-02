# js/flake.nix
{
  description = "JS flake using pnpm";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        jsTools = [
          pkgs.nodejs_20
          pkgs.nodePackages.pnpm
          pkgs.just
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          packages = jsTools;
        };
      }
    );
}
