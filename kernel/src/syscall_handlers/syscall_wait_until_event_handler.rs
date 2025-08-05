use core::{num::NonZero, slice};

use alloc::boxed::Box;
use common::{LOWER_HALF_END, Syscall, SyscallWaitUntilEvent, SyscallWaitUntilEventError};

use crate::{
    CAPABILITIES,
    cpu_local_data::get_local,
    run_tasks::run_threads,
    task::{THREADS, ThreadState, ThreadWaitingState},
    try_access_user_mem::try_access_user_mem,
};

use super::GenericSyscallHandler;

pub struct SyscallWaitUntilEventHandler;
impl GenericSyscallHandler for SyscallWaitUntilEventHandler {
    type S = SyscallWaitUntilEvent;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Return(<SyscallWaitUntilEvent as Syscall>::Output),
            RunTasks,
        }
        let get_action = (|| {
            let input = helper.input();
            if input.len() == 0 {
                return Action::Return(Err(SyscallWaitUntilEventError::Empty));
            }
            if input.pointer() == 0
                || input.pointer() + input.len() * (size_of::<u64>() as u64) > LOWER_HALF_END
            {
                return Action::Return(Err(SyscallWaitUntilEventError::InvalidEventsPtr));
            }
            let events_ptr = input.pointer() as *mut u64;
            let events_len = input.len() as usize;
            let events = match try_access_user_mem(|| {
                let slice = unsafe { slice::from_raw_parts(events_ptr, events_len) };
                Box::new(Box::<[u64]>::from(slice))
            }) {
                Ok(data) => data,
                Err(_e) => {
                    return Action::Return(Err(SyscallWaitUntilEventError::InvalidEventsPtr));
                }
            };

            let threads = THREADS.read();
            let local = get_local();
            let mut running_thread = local.running_thread.lock();
            let current_thread = threads.get(&running_thread.unwrap()).unwrap();

            let mut events_pushed = 0;
            let capabilities = CAPABILITIES.read();
            for event in events.iter() {
                if let Some(capability_id) = NonZero::new(*event) {
                    let capability = if let Some(capability) = capabilities.get(&capability_id) {
                        capability
                    } else {
                        return Action::Return(Err(SyscallWaitUntilEventError::EventNotFound));
                    };
                    if capability.process_id != current_thread.process.id.into() {
                        return Action::Return(Err(SyscallWaitUntilEventError::EventNotFound));
                    }
                    let event_already_happened = if let Some(event_already_happened) =
                        capability._type.take_pending_event()
                    {
                        event_already_happened
                    } else {
                        return Action::Return(Err(SyscallWaitUntilEventError::InvalidCapability));
                    };
                    if event_already_happened {
                        let event_ptr = unsafe { events_ptr.add(events_pushed) };
                        if let Err(_e) = try_access_user_mem(|| {
                            unsafe { event_ptr.write(*event) };
                            Box::new(())
                        }) {
                            return Action::Return(Err(
                                SyscallWaitUntilEventError::InvalidEventsPtr,
                            ));
                        };
                        events_pushed += 1;
                    }
                } else {
                    return Action::Return(Err(SyscallWaitUntilEventError::CapabilityZero));
                }
            }

            if let Some(events_pushed) = NonZero::new(events_pushed) {
                Action::Return(Ok(events_pushed))
            } else {
                *current_thread.state.write() = ThreadState::WaitingForEvents(ThreadWaitingState {
                    saved_regs: helper.saved_regs().clone(),
                    events_slice: *input,
                    events: events
                        .into_iter()
                        .map(|event| (event.try_into().unwrap(), false))
                        .collect(),
                });
                *running_thread = None;
                Action::RunTasks
            }
        })();
        match get_action {
            Action::Return(value) => helper.syscall_return(&value),
            Action::RunTasks => run_threads(),
        }
    }
}
