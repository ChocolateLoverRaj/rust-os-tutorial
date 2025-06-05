use common::SyscallExit;

use super::GenericSyscallHandler;

pub struct SyscallExitHandler;
impl GenericSyscallHandler for SyscallExitHandler {
    type S = SyscallExit;
    fn handle_syscall_simple(_input: super::SyscallInput<Self::S>) -> ! {
        todo!("Syscall exit called. Terminate process.")
    }
}
