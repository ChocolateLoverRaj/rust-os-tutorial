#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn _start() {
    naked_asm!(
        "
        // Entry point for the kernel. Registers:
        // x0 -> 32 bit pointer to DTB in memory (primary core only) / 0 (secondary cores)
        // x1 -> 0
        // x2 -> 0
        // x3 -> 0
        // x4 -> 32 bit kernel entry point, _start location
        // Set the stack pointer to the stack space we reserved in the linker script
        ldr x5, =__stack_top
        mov sp, x5

        // clear bss
        ldr x5, =__bss_start
        ldr w6, =__bss_size
        1:
            cbz     w6, 2f
            str     xzr, [x5], #8
            sub     w6, w6, #1
            cbnz    w6, 1b
        2:
            bl      {kernel_main}
        ",
        kernel_main = sym kernel_main_64
    )
}

fn kernel_main_64(dtb_ptr: u32, _x1: usize, _x2: usize, _x3: usize) -> ! {
    use aarch64_cpu::registers::{MIDR_EL1, Readable};

    logger::init();

    info!(
        "Hello from Rust kernel booted on 64 bit ARM. This is likely booted on a Raspberry Pi 3 or 4. Device Tree pointer: {dtb_ptr:#X}."
    );

    // TODO: Use device tree
    let midr = MIDR_EL1.get();
    init_common(u12::from_u64(MIDR_EL1::PartNum.read(midr)));

    halt_loop()
}
