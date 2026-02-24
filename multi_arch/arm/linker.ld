SECTIONS
{
    __kernel_start = .;

    .text :
    {
        /* We are following the Linux 32-bit ARM boot protocol. The format must be a zImage */
        /* The kernel is jumped to the first byte of the zImage. The first 36 bytes are for instructions */
        KEEP(*(.text._header));
        ASSERT(. <= __kernel_start + 0x24, "header instructions don't fit");

        /* At 0x24, there is a u32 which is a magic value */
        . = __kernel_start + 0x24;
        /* MAGIC */
        LONG(0x016f2818);
        /* Then there is the start address in memory where the kernel will start */
        LONG(__kernel_start);
        /* Then there is the end address in memory (exlusive) where the kernel will end */
        LONG(__kernel_end);
        /* Then 4 x u32 are reserved / unused */
        LONG(0);
        LONG(0);
        LONG(0);
        LONG(0);
        /* After that we can have whatever we want */

        *(.text .text.*);
    }

    .rodata :
    {
        *(.rodata .rodata.*)
    }

    .rel.dyn : {
        . = ALIGN(4);
        __rel_start = . - __kernel_start;
        *(.rel*)
        __rel_end = . - __kernel_start;
    }

    .data :
    {
        *(.data)
    }

    .bss :
    {
        . = ALIGN(16);
        __bss_start = . - __kernel_start;
        *(.bss, .bss.*)
        . = ALIGN(16);
        __bss_end = . - __kernel_start;
    }

    . = ALIGN(8);
    . += 1 * 1024 * 1024;
    __stack_top = . - __kernel_start;

    . = ALIGN(8);
    . += 128 * 1024;
    __interrupt_handler_stack_top = . - __kernel_start;

    __kernel_end = .;
}
