use abi::Syscall;

use super::*;

pub type SyscallHandler = dyn Fn(SyscallData) -> !;

pub fn get_handler(syscall_number: Syscall) -> &'static SyscallHandler {
    match syscall_number {
        Syscall::HelloWorld => &s_hello_world,
    }
}
