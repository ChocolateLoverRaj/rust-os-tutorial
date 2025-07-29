use core::sync::atomic::Ordering;

use alloc::boxed::Box;
use common::SyscallWaitUntilEvent;
use nodit::interval::ie;

use crate::{
    cpu_local_data::get_local,
    event_stream_mem::EventStreamMem,
    run_tasks::run_threads,
    task::{CHANNELS, PS2_EVENT_STREAMS, THREADS, ThreadState, ThreadWaitingState},
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
            let event_streams = PS2_EVENT_STREAMS.read();
            let channels = CHANNELS.read();
            for event in &input_events {
                // TODO: Improve what happens if the event already happened. Maybe user space can directly check which events happened by itself.
                if let Some(event_stream) = event_streams.get(event) {
                    if event_stream.process.id != current_thread.process.id {
                        Err(())?;
                    }
                    let mem = event_stream.ptr as *const EventStreamMem;
                    let mem = unsafe { mem.as_ref() }.unwrap();
                    if mem.read_count.load(Ordering::Relaxed)
                        < mem.write_count.load(Ordering::Relaxed)
                    {
                        events[events_pushed] = *event;
                        events_pushed += 1;
                    }
                } else if let Some(channel) = channels.get(event) {
                    if channel.receiver != current_thread.process.id {
                        Err(())?;
                    }
                    if channel.pending_event.swap(false, Ordering::Relaxed) {
                        events[events_pushed] = *event;
                        events_pushed += 1;
                    }
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
