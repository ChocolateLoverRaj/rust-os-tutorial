#![no_std]
#![no_main]

use core::sync::atomic::Ordering;

use alloc::string::ToString;
use common::{Syscall, SyscallExit, log};
use global_allocator::FAILED_TO_ALLOCATE;
use syscalls::{syscall_exists, syscall_exit, syscall_log};

extern crate alloc;

pub mod global_allocator;
pub mod syscalls;

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    if FAILED_TO_ALLOCATE.load(Ordering::Acquire) {
        syscall_log(log::Level::Error, "panicked (and failed to allocate)");
    } else {
        syscall_log(log::Level::Error, &info.to_string());
    }
    syscall_exit()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn entry_point() -> ! {
    let should_be_true = syscall_exists(SyscallExit::ID);
    let should_be_false = syscall_exists(0);
    assert!(should_be_true);
    assert!(!should_be_false);
    syscall_log(log::Level::Info, "Hello from user mode program 🚀");
    let dynamic_message = "Allocator works".to_string();
    syscall_log(log::Level::Info, &dynamic_message);
    let mut v = alloc::vec::Vec::new();
    loop {
        v.push(49_u128);
    }
    syscall_exit()
}
