#![no_std]
#![no_main]
#![feature(stdarch_arm_hints, stdarch_arm_neon_intrinsics)]

use core::arch::arm::__wfe;
use core::arch::naked_asm;
use core::{arch::global_asm, panic::PanicInfo};

global_asm!(include_str!("./boot.S"));

#[panic_handler]
pub fn panic_handler(panic_info: &PanicInfo) -> ! {
    let _ = panic_info;
    loop {}
}

#[unsafe(no_mangle)]
extern "C" fn kernel_main() {
    loop {
        unsafe { __wfe() };
    }
}

// #[unsafe(no_mangle)]
// #[unsafe(naked)]
// extern "C" fn kernel_halt() {
//     naked_asm!("wfe")
// }
