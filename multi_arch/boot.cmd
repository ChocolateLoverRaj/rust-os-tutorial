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
else
    echo "Unknown CPU"
fi
