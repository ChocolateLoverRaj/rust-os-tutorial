#![no_std]
#![no_main]

use core::ops::DerefMut;

use alloc::string::ToString;
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
use syscalls::{syscall_exists, syscall_exit, syscall_log};

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
    syscall_exit()
}
