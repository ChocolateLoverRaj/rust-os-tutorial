#![no_std]
#![no_main]

use core::{arch::naked_asm, panic::PanicInfo, ptr::write_bytes};

unsafe extern "C" {
    static __bss: usize;
    static __bss_end: usize;
    static __stack_top: usize;
}

#[panic_handler]
fn panic_handler(_panic_info: &PanicInfo) -> ! {
    loop {}
}

extern "C" fn kernel_main() {
    unsafe { write_bytes(__bss as *mut u8, 0, __bss_end - __bss) };
    loop {}
}

#[unsafe(link_section = ".text.boot")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn boot() {
    unsafe {
        naked_asm!(
            "
                la sp, {stack_top}
                j {kernel_main}
            ",
            stack_top = sym __stack_top,
            kernel_main = sym kernel_main
        )
    }
}
