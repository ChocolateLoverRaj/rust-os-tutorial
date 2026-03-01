ENTRY(_start);

/* Linker script for a kernel running on QEMU with OpenSBI bios and -kernel option */
SECTIONS {
    . = 0xFC000000;

    .text :{
        . = ALIGN(0x1000);
        *(.text .text.*);
        . = ALIGN(0x1000);
    }

    /* `const`s */
    .rodata :{
        . = ALIGN(0x1000);
        *(.rodata .rodata.*);
        . = ALIGN(0x1000);
    }

    /* `static`s that are not zero-initialized */
    .data :{
        . = ALIGN(0x1000);
        *(.data .data.*);
        . = ALIGN(0x1000);
    }

    /* `static`s that are zero-initialized */
    .bss :{
        . = ALIGN(0x1000);
        *(.bss .bss.*);
        . = ALIGN(0x1000);
    }
}
