dhcp
wget 0x40010000 http://10.0.2.2:8982/qemu_virt/kernel_virt.img
fdt move ${fdtcontroladdr} 0x42000000
bootz 0x40010000 - ${fdtaddr}
