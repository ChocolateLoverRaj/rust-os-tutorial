SECTIONS
{
    __kernel_start = .;

    .text :
    {
        /* We are following the Linux 64-bit ARM boot protocol.
           https://docs.kernel.org/arch/arm64/booting.html
        */

        /* The kernel is jumped to the first byte of the zImage. The first 36 bytes are for instructions */
        KEEP(*(.text._header));
        ASSERT(. <= __kernel_start + 0x8, "header instructions don't fit");

        /* At 0x2, there is `text_offset`. We'll leave this at 0x0 */
        . = __kernel_start + 0x8;
        QUAD(0x0);
        /* `image_size` */
        QUAD(__kernel_end);
        /* `flags` */
        QUAD(0x0);
        /* `res2` */
        QUAD(0x0);
        /* `res3` */
        QUAD(0x0);
        /* `res4` */
        QUAD(0x0);
        /* MAGIC */
        LONG(0x644d5241);
        /* `res5` */
        LONG(0x0);

        /* After that we can have whatever we want */
        *(.text .text.*);
    }

    .rodata :
    {
        *(.rodata .rodata.*)
    }

    .rela.dyn : {
        . = ALIGN(8);
        __rel_start = . - __kernel_start;
        *(.rela*)
        __rel_end = . - __kernel_start;
    }

    .data :
    {
        *(.data)
    }

    .bss :
    {
        . = ALIGN(8);
        __bss_start = . - __kernel_start;
        *(.bss, .bss.*)
        . = ALIGN(8);
        __bss_end = . - __kernel_start;
    }

    . = ALIGN(16);
    . += 1 * 1024 * 1024;
    __stack_top = . - __kernel_start;

    . = ALIGN(16);
    . += 128 * 1024;
    __interrupt_handler_stack_top = . - __kernel_start;

    __kernel_end = .;
}

__bss_size = __bss_end - __bss_start;
