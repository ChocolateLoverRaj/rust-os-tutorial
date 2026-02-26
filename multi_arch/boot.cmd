dhcp
if test ${cpu} = "armv7"; then
    echo "Detected arm"
    wget ${kernel_addr_r} http://${serverip}:8982/kernel7.img
    fdt move ${fdtcontroladdr} ${fdt_addr}
    bootz ${kernel_addr_r} - ${fdtaddr}
elif test ${cpu} = "armv8"; then
    echo "Detected aarch64"
    wget ${kernel_addr_r} http://${serverip}:8982/kernel8.img
    booti ${kernel_addr_r} - ${fdtcontroladdr}
elif test ${arch} = "riscv"; then
    echo "Detected RISC-V"
    wget ${kernel_addr_r} http://${serverip}:8982/kernel_riscv32.img
    fdt move ${fdtcontroladdr} ${fdt_addr_r}
    booti ${kernel_addr_r} - ${fdtaddr}
else
    echo "Unknown CPU"
fi
