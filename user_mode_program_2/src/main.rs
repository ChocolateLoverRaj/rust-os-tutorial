#![no_std]
#![no_main]

extern crate alloc;

use core::arch::naked_asm;

use alloc::collections::btree_map::BTreeMap;
use common::{
    EnvEntry,
    embedded_graphics::{
        pixelcolor::Rgb888,
        prelude::{Dimensions, RgbColor},
        primitives::{PrimitiveStyleBuilder, StyledDrawable},
    },
    env_entries, log,
};
use user_lib::{CopyData, ENV_KEY, WindowSharedMemClient, logger, syscall_exit_process};

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    user_lib::panic_handler(info)
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

    let ptr = *env_entries.get(&ENV_KEY).unwrap();
    let mut window_client = unsafe { WindowSharedMemClient::new(ptr) };
    window_client
        .bounding_box()
        .draw_styled(
            &PrimitiveStyleBuilder::new().fill_color(Rgb888::RED).build(),
            &mut window_client,
        )
        .unwrap();
    window_client.update_screen(CopyData {
        x: 0,
        y: 0,
        width: window_client.bounding_box().size.width.into(),
        height: window_client.bounding_box().size.height.into(),
    });

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
