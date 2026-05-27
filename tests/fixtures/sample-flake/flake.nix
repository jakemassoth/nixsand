{
  description = "nixsand e2e test fixture";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: {
    devShells.aarch64-linux.default = nixpkgs.legacyPackages.aarch64-linux.mkShell {
      packages = [ nixpkgs.legacyPackages.aarch64-linux.hello ];
    };
  };
}
