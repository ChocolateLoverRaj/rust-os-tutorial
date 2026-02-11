#![no_std]
#![no_main]
mod logger;
mod sbi;

use core::{arch::naked_asm, panic::PanicInfo, ptr::write_bytes};

use riscv as _;

use crate::sbi::shutdown;

unsafe extern "C" {
    static __bss: usize;
    static __bss_end: usize;
    static __stack_top: usize;
}

#[panic_handler]
fn panic_handler(_panic_info: &PanicInfo) -> ! {
    loop {}
}

extern "C" fn kernel_main(hart_id: usize, ftd_ptr: usize) {
    unsafe { write_bytes(__bss as *mut u8, 0, __bss_end - __bss) };
    unsafe {
        logger::init();
    }

    log::info!("Hello from Rust!. HART ID: {hart_id}. Device Tree pointer: {ftd_ptr:#X}.");

    shutdown()
}

#[unsafe(link_section = ".text.boot")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn boot() {
    naked_asm!(
        "
                la sp, {stack_top}
                j {kernel_main}
            ",
        stack_top = sym __stack_top,
        kernel_main = sym kernel_main
    )
}
