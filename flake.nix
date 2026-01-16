{
  description = "Stahl";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default";
  };

  outputs = {
    self,
    nixpkgs,
    systems,
  }: let
    eachSystem = nixpkgs.lib.genAttrs (import systems);
    pkgsFor = nixpkgs.legacyPackages;
  in {
    packages = eachSystem (system: {
      default = self.packages.${system}.stahl;
      stahl = pkgsFor.${system}.callPackage ./nix/package.nix {
        inherit (pkgsFor.${system}.darwin.apple_sdk.frameworks) Security;
      };
    });

    formatter = eachSystem (system: pkgsFor.${system}.alejandra);

    # DEPRECATED
    legacyPackages = self.packages;
    defaultPackage = eachSystem (system: self.packages.${system}.default);

    devShells = eachSystem (system: {
      default = pkgsFor.${system}.callPackage ./nix/shell.nix {
        inherit (self.packages.${system}) stahl;
        inherit (pkgsFor.${system}.darwin.apple_sdk.frameworks) CoreServices SystemConfiguration;
      };
    });

    apps = eachSystem (system: {
      stahl = {
        type = "app";
        program =
          pkgsFor.${system}.lib.getExe
          self.packages.${system}.stahl;
      };
      default = self.apps.${system}.stahl;
    });
  };
}
