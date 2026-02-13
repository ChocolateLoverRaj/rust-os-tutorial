#!/usr/bin/env bash
set -xue

# QEMU file path
QEMU=qemu-system-riscv32

cargo build --target riscv32imc-unknown-none-elf

# Start QEMU
$QEMU -machine virt -nographic -serial mon:stdio --no-reboot \
    -kernel target/riscv32imc-unknown-none-elf/debug/kernel
