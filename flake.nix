{
  description = "Top-level flake delegating to rs and js";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    js.url = "./js";
    rs.url = "./rs";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      js,
      rs,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (system: {
      devShells.default = nixpkgs.legacyPackages.${system}.mkShell {
        inputsFrom = [
          rs.devShells.${system}.default
          js.devShells.${system}.default
        ];
      };
      packages = {
        inherit (rs.packages.${system}) moq-relay moq-clock hang;
      };
    });
}
