#![no_std]
#![no_main]
#![feature(maybe_uninit_slice)]

use core::ops::DerefMut;

use alloc::string::ToString;
use async_keyboard::AsyncKeyboard;
use async_mouse::AsyncMouse;
use common::{
    Syscall, SyscallExit,
    embedded_graphics::{
        pixelcolor::Rgb888,
        prelude::{Dimensions, WebColors},
        primitives::{PrimitiveStyleBuilder, StyledDrawable},
    },
    log,
};
use execute_future::execute_future;
use executor_context::ExecutorContext;
use frame_buffer::FrameBuffer;
use futures::{StreamExt, future::join};
use syscalls::{syscall_exists, syscall_exit, syscall_log};

extern crate alloc;

pub mod async_keyboard;
pub mod async_mouse;
pub mod execute_future;
pub mod executor_context;
pub mod frame_buffer;
pub mod global_allocator;
pub mod logger;
pub mod panic_handler;
pub mod syscalls;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point() -> ! {
    logger::init();
    let should_be_true = syscall_exists(SyscallExit::ID);
    let should_be_false = syscall_exists(0);
    assert!(should_be_true);
    assert!(!should_be_false);
    syscall_log(log::Level::Info, "Hello from user mode program 🚀");
    let dynamic_message = "Allocator works".to_string();
    syscall_log(log::Level::Info, &dynamic_message);
    let mut frame_buffer = FrameBuffer::try_new().unwrap();
    frame_buffer
        .bounding_box()
        .draw_styled(
            &PrimitiveStyleBuilder::new()
                .fill_color(Rgb888::CSS_MEDIUM_SEA_GREEN)
                .build(),
            frame_buffer.deref_mut(),
        )
        .unwrap();

    let executor_context = ExecutorContext::default();
    execute_future(
        &executor_context,
        join(
            AsyncKeyboard::new(&executor_context)
                .for_each(async |data| log::info!("Received key: {data}")),
            async {
                if let Ok(async_mouse) = AsyncMouse::new(&executor_context) {
                    async_mouse
                        .for_each(async |data| log::info!("Mouse input: {data}"))
                        .await;
                }
            },
        ),
    );
    syscall_exit();
}
