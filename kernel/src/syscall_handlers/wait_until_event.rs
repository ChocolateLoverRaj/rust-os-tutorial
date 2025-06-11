use core::ops::Deref;

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
            for event in events.deref() {
                if ![KEYBOARD_EVENT_ID, MOUSE_EVENT_ID].contains(event) {
                    Err(())?;
                }
                if *event == KEYBOARD_EVENT_ID {
                    if let Some(keyboard) = task.keyboard.as_mut() {
                        if keyboard.pending_event {
                            events
                        }
                    }
                }
            }

            Ok::<_, ()>(if keyboard.pending_event {
                keyboard.pending_event = false;
                // TODO: When there are more possible events, make sure to write to the slice which events happened
                Action::Return(1)
            } else {
                task.state = TaskState::Waiting(WaitingState {
                    events: events.iter().copied().collect(),
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
