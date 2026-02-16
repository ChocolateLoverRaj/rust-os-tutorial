SECTIONS
{
    /* Starts at LOADER_ADDR. */
    . = LOADER_ADDR;
    .text :
    {
        KEEP(*(.text._start))
        *(.text)
    }

    .rodata :
    {
        *(.rodata)
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
}
