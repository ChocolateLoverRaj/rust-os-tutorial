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
    x86_64::instructions::interrupts::int3();
    loop {
        core::hint::spin_loop();
    }
}
