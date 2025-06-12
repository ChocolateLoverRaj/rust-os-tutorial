use alloc::boxed::Box;
use common::SyscallWaitUntilEvent;
use nodit::interval::ie;
use x86_64::instructions::interrupts;

use crate::{
    hlt_loop::hlt_loop,
    syscall_handlers::{keyboard::KEYBOARD_EVENT_ID, mouse::MOUSE_EVENT_ID},
    task::{TASK, TaskState, WaitingState},
};

use super::GenericSyscallHandler;

pub struct SyscallWaitUntilEventHandler;
impl GenericSyscallHandler for SyscallWaitUntilEventHandler {
    type S = SyscallWaitUntilEvent;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Return(u64),
            Wait,
        }
        let get_action = || {
            let mut task = TASK.try_lock().unwrap();
            let task = task.as_mut().unwrap();
            let input = helper.input();

            if !task
                .mapped_virtual_memory
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
            for event in &input_events {
                if ![KEYBOARD_EVENT_ID, MOUSE_EVENT_ID].contains(event) {
                    Err(())?;
                }
                if *event == KEYBOARD_EVENT_ID
                    && let Some(keyboard) = task.keyboard.as_mut()
                    && keyboard.pending_event
                {
                    keyboard.pending_event = false;
                    events[events_pushed] = *event;
                    events_pushed += 1;
                } else if *event == MOUSE_EVENT_ID
                    && let Some(mouse) = task.mouse.as_mut()
                    && mouse.pending_event
                {
                    mouse.pending_event = false;
                    events[events_pushed] = *event;
                    events_pushed += 1;
                }
            }

            Ok::<_, ()>(if events_pushed > 0 {
                Action::Return(events_pushed as u64)
            } else {
                task.state = TaskState::Waiting(WaitingState {
                    events: input_events,
                    saved_regs: helper.saved_regs().clone(),
                    events_slice: *input,
                });
                Action::Wait
            })
        };
        match get_action() {
            Err(()) => todo!("terminate"),
            Ok(Action::Return(value)) => helper.syscall_return(&value),
            Ok(Action::Wait) => {
                log::debug!("Waiting for events");
                interrupts::enable();
                hlt_loop()
            }
        }
    }
}
