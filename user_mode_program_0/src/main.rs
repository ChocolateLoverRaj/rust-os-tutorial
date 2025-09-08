#![no_std]
#![no_main]

use core::{hint::black_box, sync::atomic::AtomicU8};

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

static TEST_VAR: AtomicU8 = AtomicU8::new(0);

#[unsafe(no_mangle)]
unsafe extern "sysv64" fn entry_point() -> ! {
    black_box(&TEST_VAR);
    unsafe {
        (0xABCDEF as *mut u8).read_volatile();
    }
    loop {
        core::hint::spin_loop();
    }
}
