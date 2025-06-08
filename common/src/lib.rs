#![no_std]
mod slice_data;
mod syscall;
mod syscall_alloc;
mod syscall_alloc_2;
mod syscall_exists;
mod syscall_exit;
mod syscall_log;
mod syscall_mem_info;

pub use slice_data::SliceData;
pub use syscall::*;
pub use syscall_alloc::*;
pub use syscall_alloc_2::*;
pub use syscall_exists::*;
pub use syscall_exit::*;
pub use syscall_log::*;
pub use syscall_mem_info::*;
