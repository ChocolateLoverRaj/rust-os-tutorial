use alloc::collections::btree_map::Entry;
use common::SyscallExitProcess;
use x2apic::lapic::IpiAllShorthand;

use crate::{
    cpu_local_data::get_local,
    interrupt_vector::InterruptVector,
    run_tasks::run_threads,
    task::{THREAD_PRIORITIES, THREADS},
};

use super::GenericSyscallHandler;

pub struct SyscallExitProcessHandler;
impl GenericSyscallHandler for SyscallExitProcessHandler {
    type S = SyscallExitProcess;
    fn handle_decoded_syscall(_input: super::SyscallHelper<Self::S>) -> ! {
        // We need to do the following (in order):
        // - Stop all threads
        // - Clean up resources used by the process (which rn is just memory)
        // We can immediately stop this thread. We can't immediately stop other threads.
        // We send an IPI to the other CPUs so that they will stop running the other threads in this process.
        // We need to at some point clean up memory once the last thread is stopped
        {
            let mut thread_priorities = THREAD_PRIORITIES.write();
            let mut threads = THREADS.write();
            let local = get_local();
            let running_thread_id = local.running_thread.lock().take().unwrap();
            let running_process_id = threads.get(&running_thread_id).unwrap().process.id;
            let mut index = 0;
            while let Some(thread_id) = thread_priorities.get(index) {
                if let Entry::Occupied(entry) = threads.entry(*thread_id) {
                    if entry.get().process.id == running_process_id {
                        thread_priorities.remove(index);
                        entry.remove();
                    } else {
                        index += 1;
                    }
                }
            }
            let mut local_apic = local.local_apic.get().unwrap().try_lock().unwrap();
            unsafe {
                local_apic.send_ipi_all(
                    InterruptVector::CheckTasks.into(),
                    IpiAllShorthand::AllExcludingSelf,
                );
            }
            // TODO: Clean up resources used by process once all threads of the process stop
            // if other_threads == 0 {
            //     MEMORY
            //         .get()
            //         .unwrap()
            //         .physical_memory
            //         .lock()
            //         .remove_user_mode_memory();
            // }
        }
        run_threads()
    }
}
