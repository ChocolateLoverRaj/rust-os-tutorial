dhcp
wget ${kernel_addr_r} http://${serverip}:8982/kernel7.img
fdt move ${fdtcontroladdr} ${fdt_addr}
bootz ${kernel_addr_r} - ${fdtaddr}
