use alloc::boxed::Box;
use common::SyscallWaitUntilEvent;
use nodit::interval::ie;

use crate::{
    cpu_local_data::get_local,
    run_tasks::run_threads,
    task::{EVENT_STREAMS, THREADS, ThreadState, ThreadWaitingState},
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
            let threads = THREADS.read();
            let local = get_local();
            let mut running_thread = local.running_thread.lock();
            let current_thread = threads.get(&running_thread.unwrap()).unwrap();
            if !current_thread
                .process
                .mapped_virtual_memory
                .read()
                .overlapping(ie(
                    input.pointer(),
                    input.pointer().saturating_add(input.len()),
                ))
                .all(|(_interval, mem)| mem.write)
            {
                Err(())?;
            }
            let events = unsafe { input.try_to_slice_mut::<u64>() }.ok_or(())?;
            let input_events = events.iter().copied().collect::<Box<_>>();
            let mut events_pushed = 0;
            let event_streams = EVENT_STREAMS.read();
            for event in &input_events {
                let event_stream = event_streams.get(event).ok_or(())?;
                if event_stream.process != current_thread.process.id {
                    Err(())?;
                }
                if !event_stream.queue.is_empty() {
                    events[events_pushed] = *event;
                    events_pushed += 1;
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
                        .map(|event| (event, false))
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
