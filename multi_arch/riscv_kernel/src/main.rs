#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic_handler(panic_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    sbi::legacy::console_putchar(b'k');
    loop {}
}
