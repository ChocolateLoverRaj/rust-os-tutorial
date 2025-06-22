use common::SyscallGetThreadId;

use crate::cpu_local_data::get_local;

use super::GenericSyscallHandler;

pub struct SyscallGetThreadIdHandler;
impl GenericSyscallHandler for SyscallGetThreadIdHandler {
    type S = SyscallGetThreadId;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let r = {
            let thread_id = get_local().running_thread.try_lock().unwrap().unwrap();
            thread_id.into()
        };
        helper.syscall_return(&r)
    }
}
