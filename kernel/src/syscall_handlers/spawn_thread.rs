use common::{SpawnThreadRelativePriority, Syscall, SyscallSpawnThread};
use x2apic::lapic::IpiAllShorthand;

use crate::{
    cpu_local_data::get_local,
    interrupt_vector::InterruptVector,
    run_tasks::run_threads,
    task::{
        StartData, THREAD_PRIORITIES, THREADS, Thread, ThreadId, ThreadReadyState,
        ThreadReadyStateInSyscall, ThreadState,
    },
};

use super::GenericSyscallHandler;

pub struct SyscallSpawnThreadHandler;
impl GenericSyscallHandler for SyscallSpawnThreadHandler {
    type S = SyscallSpawnThread;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        {
            let mut thread_priorities = THREAD_PRIORITIES.write();
            let mut threads = THREADS.write();
            let local = get_local();
            let mut running_thread = local.running_thread.try_lock().unwrap();
            let running_thread_id = running_thread.take().unwrap();
            let running_thread_position = thread_priorities
                .iter()
                .position(|thread_id| *thread_id == running_thread_id)
                .unwrap();
            let new_thread_position = match helper.input().priority {
                SpawnThreadRelativePriority::Lower => running_thread_position + 1,
                SpawnThreadRelativePriority::Higher => running_thread_position,
            };
            let new_thread_id = ThreadId::new_unique();
            let current_thread = threads.get(&running_thread_id).unwrap();
            *current_thread.state.write() =
                ThreadState::Ready(ThreadReadyState::InSyscall(ThreadReadyStateInSyscall {
                    saved_regs: helper.saved_regs().clone(),
                    output: Self::S::encode_output(&()),
                }));
            let process = current_thread.process.clone();
            threads.insert(
                new_thread_id,
                Thread {
                    process,
                    state: spin::RwLock::new(ThreadState::Ready(ThreadReadyState::ReadyToStart(
                        StartData {
                            rip: helper.input().rip,
                            rsp: helper.input().rsp,
                        },
                    ))),
                },
            );
            thread_priorities.insert(new_thread_position, new_thread_id);
            let mut local_apic = local.local_apic.get().unwrap().try_lock().unwrap();
            log::debug!("Spawned thread. sending ipi");
            unsafe {
                local_apic.send_ipi_all(
                    u8::from(InterruptVector::CheckTasks),
                    IpiAllShorthand::AllExcludingSelf,
                )
            };
        }
        run_threads()
    }
}
