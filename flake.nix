{
  description = "MoQ";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = inputs@{ flake-utils, ... }:
    flake-utils.lib.meld inputs [
      ./nix/modules
      ./nix/packages/moq.nix
      ./nix/overlay.nix
      ./nix/shell.nix
    ];
}
