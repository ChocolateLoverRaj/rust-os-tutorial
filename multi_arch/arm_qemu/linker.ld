SECTIONS
{
    /* Starts at LOADER_ADDR. */
    . = 0x40010000;

    __kernel_start = .;

    .text :
    {
        KEEP(*(.text._start));
        *(.text .text.*);
    }

    .rodata :
    {
        *(.rodata .rodata.*)
    }

    .data :
    {
        *(.data)
    }

    .bss :
    {
        . = ALIGN(16);
        __bss_start = .;
        *(.bss)
        . = ALIGN(16);
        __bss_end = .;
    }

    . = ALIGN(8);
    . += 128 * 1024;
    __stack_top = .;

    . = ALIGN(8);
    . += 128 * 1024;
    __interrupt_handler_stack_top = .;

    .ARM.exidx : {
        *(.ARM.exidx)
    }

    __kernel_end = .;
}
