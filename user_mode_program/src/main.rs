#![no_std]
#![no_main]

use alloc::string::ToString;
use common::{Syscall, SyscallExit, log};
use syscalls::{syscall_exists, syscall_log};

extern crate alloc;

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
    let mut v = alloc::vec::Vec::new();
    loop {
        v.push(u128::default());
    }
}
