use core::mem::MaybeUninit;

use common::{SyscallReadKeyboard, SyscallSubscribeToKeyboard};
use crossbeam_queue::ArrayQueue;
use nodit::Interval;

use crate::task::{TASK, TaskKeyboard};

use super::GenericSyscallHandler;

pub const KEYBOARD_EVENT_ID: u64 = 0x07D83BFBB6EB4FA9;

pub struct SyscallSubscribeToKeyboardHandler;
impl GenericSyscallHandler for SyscallSubscribeToKeyboardHandler {
    type S = SyscallSubscribeToKeyboard;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        {
            let mut task = TASK.try_lock().unwrap();
            let task = task.as_mut().unwrap();
            if task.keyboard.is_none() {
                task.keyboard = Some(TaskKeyboard {
                    queue: ArrayQueue::new(64),
                    pending_event: false,
                });
            }
        }
        helper.syscall_return(&KEYBOARD_EVENT_ID)
    }
}

pub struct SyscallReadKeyboardHandler;
impl GenericSyscallHandler for SyscallReadKeyboardHandler {
    type S = SyscallReadKeyboard;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Terminate,
            Return(u64),
        }
        let action = {
            let buffer = helper.input();
            if buffer.len() > 0 {
                let range = buffer.pointer()..=buffer.pointer().saturating_add(buffer.len() - 1);
                let mut task = TASK.try_lock().unwrap();
                let task = task.as_mut().unwrap();
                if let Some(keyboard) = task.keyboard.as_ref() {
                    let is_valid = task
                        .mapped_virtual_memory
                        .overlapping(Interval::from(range))
                        .all(|(_interval, permissions)| permissions.write);
                    if is_valid {
                        let slice = unsafe { buffer.to_slice_mut::<MaybeUninit<u8>>() };
                        let mut count = 0;
                        for slot in slice {
                            if let Some(item) = keyboard.queue.pop() {
                                slot.write(item);
                                count += 1;
                            } else {
                                break;
                            }
                        }
                        Action::Return(count)
                    } else {
                        Action::Terminate
                    }
                } else {
                    Action::Terminate
                }
            } else {
                Action::Return(0)
            }
        };
        match action {
            Action::Return(value) => helper.syscall_return(&value),
            Action::Terminate => todo!(),
        }
    }
}
