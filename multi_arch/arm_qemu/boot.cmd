echo "Hello World from U-Boot!"
dhcp
wget 0x40010000 http://10.0.2.2:8080/kernel.img
fdt move ${fdtcontroladdr} 0x42000000
bootz 0x40010000 - ${fdtaddr}
