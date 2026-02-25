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
pub extern "C" fn _start() {
    naked_asm!(
        "
        .halt:
            b .halt
        // Get the addres where the start of our kernel was loaded

        // sub r3, pc, #8
        ",
        // start_common = sym start_common
    )
}
