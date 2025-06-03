#![no_std]
#![no_main]

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point() -> ! {
    fn is_even(n: u64) -> bool {
        n % 2 == 0
    }
    is_even(3);
    loop {
        core::hint::spin_loop();
    }
}
