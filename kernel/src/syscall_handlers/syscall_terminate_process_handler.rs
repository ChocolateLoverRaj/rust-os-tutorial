use core::num::NonZeroU32;

use alloc::collections::btree_map::Entry;
use common::SyscallTerminateProcess;
use x2apic::lapic::IpiAllShorthand;

use crate::{
    cpu_local_data::get_local,
    interrupt_vector::InterruptVector,
    task::{THREAD_PRIORITIES, THREADS},
};

use super::GenericSyscallHandler;

pub struct SyscallTerminateProcessHandler;
impl GenericSyscallHandler for SyscallTerminateProcessHandler {
    type S = SyscallTerminateProcess;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = {
            // FIXME: Check if the process is trying to terminate itself
            // TODO: Restrict which processes can terminate which processes
            let process_to_terminate = *helper.input();
            let mut thread_priorities = THREAD_PRIORITIES.write();
            let mut threads = THREADS.write();
            let mut index = 0;
            let mut removed = false;
            while let Some(thread_id) = thread_priorities.get(index) {
                if let Entry::Occupied(thread) = threads.entry(*thread_id)
                    && NonZeroU32::from(thread.get().process.id) == process_to_terminate
                {
                    thread_priorities.remove(index);
                    thread.remove();
                    removed = true;
                } else {
                    index += 1;
                }
            }
            let mut local_apic = get_local().local_apic.get().unwrap().try_lock().unwrap();
            unsafe {
                local_apic.send_ipi_all(
                    InterruptVector::CheckTasks.into(),
                    IpiAllShorthand::AllExcludingSelf,
                )
            };
            removed
        };
        helper.syscall_return(&output)
    }
}
