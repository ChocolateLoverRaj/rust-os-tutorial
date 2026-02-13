SECTIONS {
    . = 0x80200000;

    .text :{
        KEEP(*(.text.boot));
        *(.text .text.*);
    }

    .rodata :{
        *(.rodata .rodata.*);
    }

    .data :{
        *(.data .data.*);
    }

    .bss :{
        __bss = .;
        *(.bss .bss.* .sbss .sbss.*);
        __bss_end = .;
    }

    . = ALIGN(16);
    . += 128 * 1024; /* 128KB */
    __stack_top = .;
}
