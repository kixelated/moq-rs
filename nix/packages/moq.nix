{ self, nixpkgs, flake-utils, crate2nix, ... }:
flake-utils.lib.eachDefaultSystem (system:
  let
    pkgs = nixpkgs.legacyPackages.${system};
    moq-relay-version = (pkgs.lib.importTOML ../../moq-relay/Cargo.toml).package.version;
    cargoWorkspace = pkgs.callPackage ../../Cargo.nix {
      defaultCrateOverrides = pkgs.defaultCrateOverrides // {
        aws-lc-rs = _: {
          # NOTE: If aws-lc-sys version changes we will need to change these variables
          DEP_AWS_LC_0_21_2_INCLUDE = "";
          DEP_AWS_LC_0_21_2_LIBCRYPTO = "";
          DEP_AWS_LC_0_21_2_ROOT = "";
          features = [
            "aws-lc-sys"
          ];
        };
      };
    };
  in
    with pkgs;
    {
      packages = lib.concatMapAttrs (name: _: {
        ${name} = cargoWorkspace.workspaceMembers.${name}.build;
      }) cargoWorkspace.workspaceMembers;
    }
)
