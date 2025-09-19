use abi::SyscallNumber;

use super::*;

pub type SyscallHandler = dyn Fn(SyscallData) -> !;

pub fn get_handler(syscall_number: SyscallNumber) -> &'static SyscallHandler {
    match syscall_number {
        SyscallNumber::HelloWorld => &s_hello_world,
        SyscallNumber::Log => &s_log,
    }
}
