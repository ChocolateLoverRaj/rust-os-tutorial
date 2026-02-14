#![no_std]
#![no_main]

mod logger;

use core::arch::naked_asm;
use core::panic::PanicInfo;

use sbi::legacy::{console_putchar, shutdown};

// These variables are defined in the linker script
unsafe extern "C" {
    static __bss_start: usize;
    static __bss_end: usize;
    static __stack_top: usize;
}

/// OpenSBI passes the HART ID in the `a0` register and a pointer to the device tree in the `a1`
/// register. Since we don't modify those registers, we can just jump to `kernel_main` and those
/// two inputs will be passed to it.
#[unsafe(link_section = ".text._start")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn _start() {
    naked_asm!(
        "
            la sp, {stack_top}
            j {kernel_main}
        ",
        stack_top = sym __stack_top,
        kernel_main = sym kernel_main
    )
}

#[panic_handler]
fn panic_handler(_panic_info: &PanicInfo) -> ! {
    sbi::legacy::console_putchar(b'p');
    loop {}
}

extern "C" fn kernel_main(hart_id: usize, ftd_ptr: usize) -> ! {
    let bss_start = {
        // Safety: it's const
        unsafe { __bss_start }
    };
    let bss_end = {
        // Safety: it's const
        unsafe { __bss_end }
    };
    let bss = bss_start as *mut u8;
    let bss_len = bss_end - bss_start;
    // Safety: we need to zero the BSS before using any static mut functions
    unsafe { bss.write_bytes(0, bss_len) };

    // Safety: we're only calling this once
    unsafe {
        logger::init();
    }

    defmt_or_log::info!(
        "Hello from Rust kernel. HART ID: {}. FTD pointer: {:#X}",
        hart_id,
        ftd_ptr
    );
    shutdown()
}
