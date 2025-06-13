use alloc::collections::btree_set::BTreeSet;
use common::{Syscall, SyscallWaitUntilEvent};
use x86_64::instructions::port::Port;

use crate::{
    cpu_local_data::get_local,
    interrupted_context::InterruptedContext,
    syscall_saved_regs::SyscallSavedRegs,
    task::{EventStreamSource, TASK, TaskState},
};

/// # Safety
/// Must be called from an actual PS/2 interrupt handler
pub unsafe fn ps2_interrupt_handler(
    interrupted_context: &mut InterruptedContext,
    ps2_source: EventStreamSource,
) -> ! {
    struct RestoreSyscallData {
        saved_regs: SyscallSavedRegs,
        output: [u64; 7],
    }
    enum Action {
        RestoreInterrupted(InterruptedContext),
        RestoreSyscall(RestoreSyscallData),
    }
    let action = {
        let mut port = Port::<u8>::new(0x60);
        let data = unsafe { port.read() };
        let local = get_local();

        let mut local_apic = local.local_apic.get().unwrap().try_lock().unwrap();
        unsafe { local_apic.end_of_interrupt() };

        let mut task = TASK.try_lock().unwrap();
        if let Some(task) = task.as_mut() {
            match &task.state {
                TaskState::Running => {
                    for event_stream in task.event_streams.values_mut() {
                        if event_stream.source == ps2_source {
                            event_stream.queue.push(data);
                            event_stream.pending_event = true;
                        }
                    }
                    Action::RestoreInterrupted(interrupted_context.clone())
                }
                TaskState::Waiting(waiting_state) => {
                    let events =
                        unsafe { waiting_state.events_slice.try_to_slice_mut::<u64>() }.unwrap();
                    let input_events = events.iter().copied().collect::<BTreeSet<_>>();
                    let mut count = 0;
                    for (id, event_stream) in &mut task.event_streams {
                        if event_stream.source == ps2_source {
                            event_stream.queue.push(data);
                            event_stream.pending_event = true;
                            if input_events.contains(id) {
                                events[count] = *id;
                                count += 1;
                            }
                        }
                    }
                    if count > 0 {
                        let action = Action::RestoreSyscall(RestoreSyscallData {
                            saved_regs: waiting_state.saved_regs.clone(),
                            output: SyscallWaitUntilEvent::encode_output(&(count as u64)),
                        });
                        task.state = TaskState::Running;
                        action
                    } else {
                        Action::RestoreInterrupted(interrupted_context.clone())
                    }
                }
            }
        } else {
            Action::RestoreInterrupted(interrupted_context.clone())
        }
    };
    match action {
        Action::RestoreInterrupted(full_context) => unsafe { full_context.restore() },
        Action::RestoreSyscall(RestoreSyscallData { saved_regs, output }) => unsafe {
            saved_regs.sysretq(output)
        },
    }
}
