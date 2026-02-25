#![no_std]
#![no_main]
#![feature(stdarch_arm_hints)]
#![cfg_attr(target_arch = "arm", feature(stdarch_arm_neon_intrinsics))]

// mod logger;

use core::{arch::naked_asm, panic::PanicInfo};

unsafe extern "C" {
    static __interrupt_handler_stack_top: usize;
}

#[panic_handler]
pub fn panic_handler(panic_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(link_section = ".text._header")]
// Prevent this function from being removed
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() {
    naked_asm!(
        "
        b {start}
        ",
        start = sym start
    )
}

#[unsafe(naked)]
pub unsafe extern "C" fn start() {
    naked_asm!(
        "
        // Set stack pointer
        adr x5, __stack_top
        mov sp, x5

        // Clear bss
        adr x5, __bss_start
        ldr w6, __bss_size
        1:
            cbz     w6, 2f
            str     xzr, [x5], #8
            sub     w6, w6, #1
            cbnz    w6, 1b

        2:
            bl      {kernel_main}
        ",
        kernel_main = sym kernel_main
    )
}

unsafe extern "C" fn kernel_main(fdt_addr: usize) -> ! {
    semihosting::println!("Hello from kernel (written in Rust) on aarch64");
    loop {}
}
