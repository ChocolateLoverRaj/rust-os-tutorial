#![no_std]
#![no_main]
#![feature(maybe_uninit_slice)]

use core::ops::DerefMut;

use async_keyboard::AsyncKeyboard;
use async_mouse::AsyncMouse;
use common::{
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
use syscalls::syscall_exit;

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
                .for_each(async |data| log::debug!("Received key: {data}")),
            async {
                if let Ok(async_mouse) = AsyncMouse::new(&executor_context) {
                    async_mouse
                        .for_each(async |data| log::debug!("Mouse input: {data}"))
                        .await;
                }
            },
        ),
    );
    syscall_exit();
}
