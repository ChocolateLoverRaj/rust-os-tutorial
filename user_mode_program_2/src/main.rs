#![no_std]
#![no_main]

extern crate alloc;

use core::arch::naked_asm;

use alloc::collections::btree_map::BTreeMap;
use common::{EnvEntry, env_entries, log};
use user_lib::{async_channel::Sender, logger, syscall_exit_process};

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn main(initial_rsp: *mut ()) -> ! {
    logger::init();
    log::debug!("Hello from user mode program 2");

    let env_entries = unsafe { env_entries(initial_rsp).as_mut() };

    log::debug!("Env entries: {env_entries:#X?}");

    let env_entries = env_entries
        .iter()
        .copied()
        .map(|EnvEntry { key, value }| (key, value))
        .collect::<BTreeMap<_, _>>();

    let channel_id = *env_entries.get(&0).unwrap();
    let mut sender = unsafe { Sender::from_channel_id(channel_id) };
    for i in 0..1_000 {
        log::debug!("{i}");
    }
    sender.send();

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
