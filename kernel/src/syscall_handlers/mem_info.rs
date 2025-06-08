use common::SyscallMemInfo;

use crate::run_user_mode_program::TASK;

use super::GenericSyscallHandler;

pub struct SyscallMemInfoHandler;
impl GenericSyscallHandler for SyscallMemInfoHandler {
    type S = SyscallMemInfo;
    fn handle_decoded_syscall(input: super::SyscallHelper<Self::S>) -> ! {
        input.syscall_return(&{
            // Drop lock
            TASK.lock().as_ref().unwrap().mem_info.clone()
        })
    }
}
