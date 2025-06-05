use common::SyscallExists;

use super::{GenericSyscallHandler, SyscallInput};

pub struct SyscallExistsHandler;
impl GenericSyscallHandler for SyscallExistsHandler {
    type S = SyscallExists;
    fn handle_decoded_syscall(input: SyscallInput<Self::S>) -> ! {
        input.syscall_return(&input.handler_exists(input.input()))
    }
}
