use common::{SyscallGetThreadId, SyscallGetThreadIdOutput};

use crate::{cpu_local_data::get_local, task::THREADS};

use super::GenericSyscallHandler;

pub struct SyscallGetThreadIdHandler;
impl GenericSyscallHandler for SyscallGetThreadIdHandler {
    type S = SyscallGetThreadId;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let r = {
            let thread_id = get_local().running_thread.try_lock().unwrap().unwrap();
            let threads = THREADS.read();
            let thread = threads.get(&thread_id).unwrap();
            SyscallGetThreadIdOutput {
                thread_id: thread_id.into(),
                process_id: thread.process.id.into(),
            }
        };
        helper.syscall_return(&r)
    }
}
