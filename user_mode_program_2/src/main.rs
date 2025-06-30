#![no_std]
#![no_main]

use common::log;
use user_lib::{logger, syscall_exit_process};

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn main() {
    logger::init();

    log::info!("Hello from user mode program 2");
}

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point() -> ! {
    main();
    syscall_exit_process()
}
