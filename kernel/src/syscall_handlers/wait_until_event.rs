use core::num::NonZero;

use alloc::boxed::Box;
use common::SyscallWaitUntilEvent;
use nodit::interval::ie;

use crate::{
    CAPABILITIES,
    cpu_local_data::get_local,
    run_tasks::run_threads,
    task::{THREADS, ThreadState, ThreadWaitingState},
};

use super::GenericSyscallHandler;

pub struct SyscallWaitUntilEventHandler;
impl GenericSyscallHandler for SyscallWaitUntilEventHandler {
    type S = SyscallWaitUntilEvent;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Return(u64),
            RunTasks,
        }
        let get_action = || {
            let input = helper.input();
            if input.len() == 0 {
                Err(())?;
            }
            let threads = THREADS.read();
            let local = get_local();
            let mut running_thread = local.running_thread.lock();
            let current_thread = threads.get(&running_thread.unwrap()).unwrap();
            // log::debug!("Thread: {current_thread:?}");
            if !current_thread
                .process
                .memory
                .read()
                .mapped_virtual_memory
                .overlapping(ie(
                    input.pointer(),
                    input.pointer().saturating_add(input.len()),
                ))
                .all(|(_interval, permissions)| permissions.permissions().write)
            {
                Err(())?;
            }
            let events = unsafe { input.try_to_slice_mut::<u64>() }.ok_or(())?;
            let input_events = events.iter().copied().collect::<Box<_>>();
            let mut events_pushed = 0;
            let capabilities = CAPABILITIES.read();
            for event in &input_events {
                if let Some(capability_id) = NonZero::new(*event) {
                    let capability = capabilities.get(&capability_id).ok_or(())?;
                    if capability.process_id != current_thread.process.id.into() {
                        Err(())?;
                    }
                    if capability._type.take_pending_event().ok_or(())? {
                        events[events_pushed] = *event;
                        events_pushed += 1;
                    }
                } else {
                    Err(())?;
                }
            }

            Ok::<_, ()>(if events_pushed > 0 {
                Action::Return(events_pushed as u64)
            } else {
                *current_thread.state.write() = ThreadState::WaitingForEvents(ThreadWaitingState {
                    saved_regs: helper.saved_regs().clone(),
                    events_slice: *input,
                    events: input_events
                        .into_iter()
                        .map(|event| (event.try_into().unwrap(), false))
                        .collect(),
                });
                *running_thread = None;
                Action::RunTasks
            })
        };
        match get_action() {
            Err(()) => todo!("terminate"),
            Ok(Action::Return(value)) => helper.syscall_return(&value),
            Ok(Action::RunTasks) => run_threads(),
        }
    }
}
