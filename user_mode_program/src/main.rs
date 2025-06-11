#![no_std]
#![no_main]
#![feature(maybe_uninit_slice)]

use core::{mem::MaybeUninit, ops::DerefMut};

use alloc::{format, string::ToString};
use common::{
    Syscall, SyscallExit,
    embedded_graphics::{
        pixelcolor::Rgb888,
        prelude::{Dimensions, WebColors},
        primitives::{PrimitiveStyleBuilder, StyledDrawable},
    },
    log,
};
use frame_buffer::FrameBuffer;
use syscalls::{
    syscall_exists, syscall_log, syscall_read_keyboard, syscall_read_mouse,
    syscall_subscribe_to_keyboard, syscall_subscribe_to_mouse, syscall_wait_until_event,
};

extern crate alloc;

pub mod frame_buffer;
pub mod global_allocator;
pub mod panic_handler;
pub mod syscalls;

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point() -> ! {
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

    if let Ok(mouse_event) = syscall_subscribe_to_mouse() {
        let mut buffer = [MaybeUninit::uninit(); 64];
        loop {
            let input = syscall_read_mouse(&mut buffer);
            if !input.is_empty() {
                syscall_log(
                    log::Level::Debug,
                    &format!("Received mouse input: {input:?}"),
                );
            }
            syscall_wait_until_event(&mut [mouse_event]);
        }
    }

    let keyboard_event = syscall_subscribe_to_keyboard();
    let mut buffer = [MaybeUninit::uninit(); 64];
    loop {
        let input = syscall_read_keyboard(&mut buffer);
        if !input.is_empty() {
            syscall_log(log::Level::Debug, &format!("Received input: {input:?}"));
        }
        syscall_wait_until_event(&mut [keyboard_event]);
    }
}
