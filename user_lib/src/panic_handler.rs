use alloc::string::ToString;
use core::fmt::Write;

use common::log;

use crate::{
    global_allocator::ALLOC_ERROR,
    syscalls::{syscall_exit_process, syscall_log},
};

pub fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    if let Some(alloc_error) = ALLOC_ERROR.get() {
        let mut s = heapless::String::<0x400>::default();
        let _ = write!(s, "panicked while allocating: {alloc_error:#?}");
        syscall_log(log::Level::Error, &s);
    } else {
        syscall_log(log::Level::Error, &info.to_string());
    }
    syscall_exit_process()
}
