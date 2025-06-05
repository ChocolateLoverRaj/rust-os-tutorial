#![no_std]
mod syscall;
mod syscall_exists;
mod syscall_exit;

pub use syscall::*;
pub use syscall_exists::*;
pub use syscall_exit::*;
