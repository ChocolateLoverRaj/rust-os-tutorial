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
    fdt addr ${fdtcontroladdr}
    fdt get value isa_base /cpus/cpu@0 riscv,isa-base
    if test ${isa_base} = "rv32i"; then
        echo "Detected RISC-V 32"
        wget ${kernel_addr_r} http://${serverip}:8982/kernel_0_riscv32.img
        wget ${ramdisk_addr_r} http://${serverip}:8982/kernel_1_riscv32
        fdt move ${fdtcontroladdr} ${fdt_addr_r}
        booti ${kernel_addr_r} ${ramdisk_addr_r}:${filesize} ${fdtaddr}
    elif test ${isa_base} = "rv64i"; then
        echo "Detected RISC-V 64"
        wget ${kernel_addr_r} http://${serverip}:8982/kernel_riscv64.img
        fdt move ${fdtcontroladdr} ${fdt_addr_r}
        booti ${kernel_addr_r} - ${fdtaddr}
    fi
else
    echo "Unknown CPU"
fi
