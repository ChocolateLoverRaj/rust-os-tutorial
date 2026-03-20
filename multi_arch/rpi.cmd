wget 0x200000 http://${serverip}:8982/kernel8_no_semihosting.img
# U-Boot tries to load the kernel at 0x0 and then fails. So we patch the text_offset field to be at 2 MiB
mw.q 0x200008 0x200000
fdt addr ${fdt_addr}
booti 0x200000 - ${fdt_addr}
