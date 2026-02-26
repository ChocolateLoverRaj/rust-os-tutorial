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
          let
            uboot_riscv32 = pkgsCross.riscv32.buildUBoot {
              defconfig = "qemu-riscv32_spl_defconfig";
              env.OPENSBI = "${pkgsCross.riscv32.opensbi}/share/opensbi/ilp32/generic/firmware/fw_dynamic.bin";
              extraConfig = ''
                CONFIG_CMD_WGET=y
                CONFIG_NET_LWIP=y
              '';
              filesToInstall = [
                "spl/u-boot-spl"
                "u-boot.itb"
              ];
            };
            uboot_riscv64 = pkgsCross.riscv64.buildUBoot {
              defconfig = "qemu-riscv64_spl_defconfig";
              env.OPENSBI = "${pkgsCross.riscv64.opensbi}/share/opensbi/lp64/generic/firmware/fw_dynamic.bin";
              extraConfig = ''
                CONFIG_CMD_WGET=y
                CONFIG_NET_LWIP=y
              '';
              filesToInstall = [
                "spl/u-boot-spl"
                "u-boot.itb"
              ];
            };
          in
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
            UBOOT_RISCV32_BIOS = "${uboot_riscv32}/u-boot-spl";
            UBOOT_RISCV32 = "${uboot_riscv32}/u-boot.itb";
            UBOOT_RISCV64_BIOS = "${uboot_riscv64}/u-boot-spl";
            UBOOT_RISCV64 = "${uboot_riscv64}/u-boot.itb";
          };
      }
    );
}
