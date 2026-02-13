#!/usr/bin/env bash
set -xue

# QEMU file path
QEMU=qemu-system-riscv64

cargo build --target riscv64gc-unknown-none-elf

# Start QEMU
$QEMU -machine virt -nographic -serial mon:stdio --no-reboot \
    -kernel target/riscv64gc-unknown-none-elf/debug/kernel
