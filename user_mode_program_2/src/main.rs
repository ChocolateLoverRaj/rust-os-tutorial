#![no_std]
#![no_main]

use core::arch::naked_asm;

use common::{env_entries, log};
use user_lib::{logger, syscall_exit_process};

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn main(initial_rsp: *mut ()) -> ! {
    logger::init();

    let env_entries = unsafe { env_entries(initial_rsp).as_mut() };

    log::info!("Hello from user mode program 2");
    log::info!("Env entries: {env_entries:#X?}");
    syscall_exit_process()
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
unsafe extern "sysv64" fn entry_point() -> ! {
    naked_asm!(
        "
            mov rdi, rsp
            call {main}
        ",
        main = sym main
    )
}
