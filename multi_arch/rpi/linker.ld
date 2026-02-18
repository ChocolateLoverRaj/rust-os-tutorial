SECTIONS
{
    /* Starts at LOADER_ADDR. */
    . = LOADER_ADDR;
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

    . = ALIGN(STACK_ALIGN);
    . += 128 * 1024;
    __stack_top = .;

    . = ALIGN(STACK_ALIGN);
    . += 128 * 1024;
    __interrupt_handler_stack_top = .;

    .ARM.exidx : {
        *(.ARM.exidx)
    }
}

/* For convenience in assembly */
__bss_size = __bss_end - __bss_start;
