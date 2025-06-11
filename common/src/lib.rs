#![no_std]
#![feature(maybe_uninit_slice)]
extern crate alloc;

mod frame_buffer_embedded_graphics;
mod frame_buffer_info;
mod slice_data;
mod syscall;
mod syscall_alloc;
mod syscall_exists;
mod syscall_exit;
mod syscall_frame_buffer;
mod syscall_keyboard;
mod syscall_log;
mod syscall_mouse;
mod syscall_wait_until_event;

pub use frame_buffer_embedded_graphics::*;
pub use frame_buffer_info::*;
pub use slice_data::SliceData;
pub use syscall::*;
pub use syscall_alloc::*;
pub use syscall_exists::*;
pub use syscall_exit::*;
pub use syscall_frame_buffer::*;
pub use syscall_keyboard::*;
pub use syscall_log::*;
pub use syscall_mouse::*;
pub use syscall_wait_until_event::*;
