use common::SyscallExitThread;

use crate::{
    cpu_local_data::get_local,
    run_tasks::run_threads,
    task::{THREAD_PRIORITIES, THREADS},
};

use super::GenericSyscallHandler;

pub struct SyscallExitThreadHandler;
impl GenericSyscallHandler for SyscallExitThreadHandler {
    type S = SyscallExitThread;
    fn handle_decoded_syscall(_helper: super::SyscallHelper<Self::S>) -> ! {
        {
            let local = get_local();
            let current_thread = local.running_thread.try_lock().unwrap().take().unwrap();
            let mut thread_priorities = THREAD_PRIORITIES.write();
            let mut threads = THREADS.write();
            let position = thread_priorities
                .iter()
                .position(|thread_id| *thread_id == current_thread)
                .unwrap();
            thread_priorities.remove(position);
            threads.remove(&current_thread).unwrap();
            log::debug!("Thread {current_thread:?} exited gracefully");
        }
        run_threads()
    }
}
