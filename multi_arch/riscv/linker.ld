/* Linker script for a kernel running on QEMU with OpenSBI bios and -kernel option */
SECTIONS {
    /* This is where QEMU will tell  OpenSBI to jump to */
    __kernel_start = .;

    .text :{
        /* We are following the Linux RISC-V boot protocol: https://docs.kernel.org/arch/riscv/boot-image-header.html */

        /* code0, code1 */
        KEEP(*(.text._header));
        ASSERT(. <= __kernel_start + 0x8, "header instructions don't fit");
        . = __kernel_start + 0x8;
        /* text_offset */
        QUAD(0x0);
        /* image_size */
        QUAD(__kernel_end);
        /* flags */
        QUAD(0x0);
        /* version (0.2) */
        LONG(0x2);
        /* res1 */
        LONG(0x0);
        /* res2 */
        QUAD(0x0);
        /* magic */
        QUAD(0x5643534952);
        /* magic2 */
        LONG(0x05435352);
        /* res3 */
        LONG(0x0);

        *(.text .text.*);
    }

    /* `const`s */
    .rodata :{
        *(.rodata .rodata.*);
    }

    .rela.dyn :{
        . = ALIGN(POINTER_WIDTH);
        __rel_start = . - __kernel_start;
        *(.rela*);
        __rel_end = . - __kernel_start;
    }

    /* `static`s that are not zero-initialized */
    .data :{
        *(.data .data.*);
    }

    /* `static`s that are zero-initialized */
    .bss :{
        . = ALIGN(POINTER_WIDTH);
        __bss_start = .;
        *(.bss .bss.*);
        . = ALIGN(POINTER_WIDTH);
        __bss_end = .;
    }

    /* This is where our stack will go. This doesn't show up in the ELF, and is just memory */
    /* The stack must be aligned to 16 bytes for riscv32 and riscv64 calling conventions */
    . = ALIGN(16);
    . += 128 * 1024;

    /* Our assembly code uses this to set up the stack before calling the Rust function */
    __stack_top = . - __kernel_start;

    __kernel_end = .;
}
