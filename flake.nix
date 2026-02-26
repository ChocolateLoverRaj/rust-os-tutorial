{
  description = "A devShell example";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
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
          mkShell {
            buildInputs = [
              (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
              qemu
              cargo-binutils
              llvm
              just
              dtc
              http-server
              ubootTools
              parted
              gdb
            ];

            UBOOT_ARM =
              (pkgsCross.armv7l-hf-multiplatform.ubootQemuArm.override {
                extraConfig = ''
                  CONFIG_NET_LWIP=y
                '';
              })
              + "/u-boot.bin";
            UBOOT_AARCH64 =
              (pkgsCross.aarch64-multiplatform.ubootQemuAarch64.override {
                extraConfig = ''
                  CONFIG_NET_LWIP=y
                '';
              })
              + "/u-boot.bin";

            # UBOOT_RISCV32 =
            #   (pkgsCross.riscv32.buildUBoot {
            #     defconfig = "qemu-riscv32_defconfig";
            #     extraConfig = ''
            #       CONFIG_NET_LWIP=y
            #     '';
            #     filesToInstall = [ "u-boot.bin" ];
            #   })
            #   + "/u-boot.bin";
          };
      }
    );
}
