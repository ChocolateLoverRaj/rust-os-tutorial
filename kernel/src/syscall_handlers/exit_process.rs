use core::ops::Deref;

use common::SyscallExitProcess;

use crate::{
    cpu_local_data::get_local,
    interrupt_vector::InterruptVector,
    memory::MEMORY,
    run_tasks::run_threads,
    task::{THREAD_PRIORITIES, THREADS, ThreadState},
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
            let current_thread_id = local.running_thread.lock().unwrap();
            let current_process_id = threads.get(&current_thread_id).unwrap().process.id;
            let index = thread_priorities
                .iter()
                .position(|thread_id| *thread_id == current_thread_id)
                .unwrap();
            thread_priorities.remove(index);
            threads.remove(&current_thread_id);
            let mut other_threads = 0;
            for thread in threads.values_mut() {
                if thread.process.id == current_process_id
                    && let ThreadState::Running(local_apic_id) = thread.state.read().deref()
                {
                    unsafe {
                        local
                            .local_apic
                            .get()
                            .unwrap()
                            .try_lock()
                            .unwrap()
                            .send_ipi(InterruptVector::Preempt.into(), (*local_apic_id).into())
                    };
                    other_threads += 1;
                }
            }
            if other_threads == 0 {
                MEMORY
                    .get()
                    .unwrap()
                    .physical_memory
                    .lock()
                    .remove_user_mode_memory();
            }
        }
        run_threads()
    }
}
