# moq nix

## dev shell

To get a dev shell with cargo and ffmpeg run `nix develop`

## nix build moq binaries

To build `moq-relay`, `moq-karp` and `moq-clock` run `nix build
.#moq-relay`

## NixOS module and overlay

The moq nix flake also exports a `nixosModule` for the `moq-relay`
server and a nix overlay with the moq packages. Here is an example
NixOS configuration that uses this overlay and nixosModule:

```nix
{
  description = "My configuration";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    moq = {
      url = "github:kixelated/moq-rs";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, moq, ... }: {
    nixosConfigurations = {
      hostname = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          ./configuration.nix # Your system configuration.
          ({ pkgs, ... }: {
            nixpkgs.overlays = [ moq.overlays.default ];
            environment.systemPackages = [ pkgs.moq.moq-relay ];
          })
          moq.nixosModules.moq-relay
          {
             services.moq-relay = {
               enable = true;
               port = 4443;
               user = "moq-relay";
               group = "moq-relay";
               tls = {
                 certPath = "<cert path>";
                 keyPath = "<key path>";
               };
            };
          }
        ];
      };
    };
  };
}
```
