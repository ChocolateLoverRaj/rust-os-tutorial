use common::SyscallExit;

use super::GenericSyscallHandler;

pub struct SyscallExitHandler;
impl GenericSyscallHandler for SyscallExitHandler {
    type S = SyscallExit;
    fn handle_decoded_syscall(_input: super::SyscallHelper<Self::S>) -> ! {
        todo!("Syscall exit called. Terminate process.")
    }
}
