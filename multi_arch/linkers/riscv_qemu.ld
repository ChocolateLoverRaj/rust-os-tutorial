/* Linker script for a kernel running on QEMU with OpenSBI bios and -kernel option */
SECTIONS {
    /* This is where QEMU will tell  OpenSBI to jump to */
    . = 0x80200000;

    .text :{
        /* Our entry point  must be first */
        KEEP(*(.text._start));
        *(.text .text.*);
    }

    /* `const`s */
    .rodata :{
        *(.rodata .rodata.*);
    }

    /* `static`s that are not zero-initialized */
    .data :{
        *(.data .data.*);
    }

    /* `static`s that are zero-initialized */
    .bss :{
        __bss_start = .;
        *(.bss .bss.* .sbss .sbss.*);
        __bss_end = .;
    }

    /* This is where our stack will go. This doesn't show up in the ELF, and is just memory */
    /* The stack must be aligned to 16 bytes for riscv32 and riscv64 calling conventions */
    . = ALIGN(16);
    . += 128 * 1024;
    /* Our assembly code uses this to set up the stack before calling the Rust function */
    __stack_top = .;
}
