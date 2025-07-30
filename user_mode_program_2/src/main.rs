#![no_std]
#![no_main]

extern crate alloc;

use core::{arch::naked_asm, ptr::NonNull};

use common::{
    embedded_graphics::{
        pixelcolor::Rgb888,
        prelude::Dimensions,
        primitives::{PrimitiveStyleBuilder, StyledDrawable},
    },
    log,
};
use pc_keyboard::{HandleControl, Keyboard, ScancodeSet1, layouts::Us104Key};
use rand::{Rng, SeedableRng, rngs::SmallRng};
use user_lib::{
    CopyData, EnvEntries, ExecutorContext, KeyboardSharedMemClient, WindowSharedMemClient,
    execute_future, logger, syscall_exit_process, syscall_get_thread_id,
};

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    user_lib::panic_handler(info)
}
fn main(initial_rsp: NonNull<()>) -> ! {
    logger::init();
    let env_entries = unsafe { EnvEntries::from_initial_rsp(initial_rsp) };
    log::debug!("{env_entries:#X?}");

    let mut window_client = unsafe { WindowSharedMemClient::new(&env_entries) };
    let mut keyboard = unsafe { KeyboardSharedMemClient::new(&env_entries) }.unwrap();
    let executor_context = ExecutorContext::default();
    execute_future(&executor_context, async {
        window_client
            .bounding_box()
            .draw_styled(
                &PrimitiveStyleBuilder::new()
                    .fill_color({
                        let mut rng = SmallRng::seed_from_u64(
                            syscall_get_thread_id().process_id.get().into(),
                        );
                        Rgb888::new(rng.random(), rng.random(), rng.random())
                    })
                    .build(),
                &mut window_client,
            )
            .unwrap();
        window_client.update_screen(CopyData {
            x: 0,
            y: 0,
            width: window_client.bounding_box().size.width.into(),
            height: window_client.bounding_box().size.height.into(),
        });
        let mut client = keyboard
            .request(&executor_context, 64.try_into().unwrap())
            .await
            .unwrap();
        log::info!("Got client: {client:#?}");
        let mut keyboard = Keyboard::new(ScancodeSet1::new(), Us104Key, HandleControl::Ignore);
        loop {
            let data = client.read(&executor_context).await;
            if let Ok(Some(key_event)) = keyboard.add_byte(data) {
                log::info!("Key event: {key_event:#?}");
            }
        }
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
