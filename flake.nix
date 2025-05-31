{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      {
        devShells.default =
          with pkgs;
          let
            ovmf = OVMF.fd;
            limine = pkgs.limine.override {
              enableAll = true;
            };
          in
          mkShell {
            OVMF_PATH = "${ovmf}/FV/OVMF.fd";
            LIMINE_PATH = "${limine}/share/limine";

            nativeBuildInputs = [
              rustPlatform.bindgenHook
            ];
            buildInputs = [
              (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
              qemu
              ovmf
              rust-analyzer
              xorriso
              limine
              gdb
            ];
          };
      }
    );
}
