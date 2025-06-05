#!/usr/bin/env bash
# Prints a random 64-bit unsigned integer from /dev/random as hex (0xXXXXXXXXXXXXXXXX)

set -euo pipefail

printf '0x%016X\n' "$(od -An -N8 -t u8 /dev/random)"
