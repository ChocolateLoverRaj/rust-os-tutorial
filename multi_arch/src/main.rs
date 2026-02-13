#![no_std]
#![no_main]

use core::arch::naked_asm;
#[cfg(not(feature = "semihosting"))]
use core::panic::PanicInfo;

#[cfg(feature = "semihosting")]
use riscv as _;
use sbi::legacy::shutdown;
#[cfg(feature = "semihosting")]
use semihosting as _;

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

#[cfg(not(feature = "semihosting"))]
#[panic_handler]
fn panic_handler(_panic_info: &PanicInfo) -> ! {
    loop {}
}

extern "C" fn kernel_main(_hart_id: usize, _ftd_ptr: usize) -> ! {
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
    unsafe { bss.write_bytes(0, bss_len) };

    #[cfg(not(feature = "semihosting"))]
    for byte in "Hello from sbi_console_put_char\n"
        .as_bytes()
        .iter()
        .copied()
    {
        sbi::legacy::console_putchar(byte);
    }
    #[cfg(feature = "semihosting")]
    semihosting::println!("Hello from semihosting");
    shutdown()
}
