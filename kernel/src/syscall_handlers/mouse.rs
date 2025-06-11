use core::mem::MaybeUninit;

use common::{SyscallReadMouse, SyscallSubscribeToMouse, SyscallSubscribeToMouseError};
use crossbeam_queue::ArrayQueue;
use nodit::Interval;

use crate::task::{TASK, TaskMouse};

use super::GenericSyscallHandler;

fn try_init() -> Result<(), ()> {
    let mut ps2 = unsafe { ps2::Controller::new() };
    ps2.enable_mouse().map_err(|_| ())?;
    ps2.mouse().set_defaults().map_err(|_| ())?;
    ps2.mouse().enable_data_reporting().map_err(|_| ())?;
    Ok(())
}

pub const MOUSE_EVENT_ID: u64 = 0xDD9E0D4A9409B5F2;

pub struct SyscallSubscribeToMouseHandler;
impl GenericSyscallHandler for SyscallSubscribeToMouseHandler {
    type S = SyscallSubscribeToMouse;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = {
            let mut task = TASK.try_lock().unwrap();
            let task = task.as_mut().unwrap();
            if task.mouse.is_none() {
                match try_init() {
                    Ok(()) => {
                        task.mouse = Some(TaskMouse {
                            queue: ArrayQueue::new(64),
                            pending_event: false,
                        });
                        Ok(MOUSE_EVENT_ID)
                    }
                    Err(()) => Err(SyscallSubscribeToMouseError::NoPs2Mouse),
                }
            } else {
                Ok(MOUSE_EVENT_ID)
            }
        };
        helper.syscall_return(&output)
    }
}

pub struct SyscallReadMouseHandler;
impl GenericSyscallHandler for SyscallReadMouseHandler {
    type S = SyscallReadMouse;
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
                if let Some(mouse) = task.mouse.as_ref() {
                    let is_valid = task
                        .mapped_virtual_memory
                        .overlapping(Interval::from(range))
                        .all(|(_interval, permissions)| permissions.write);
                    if is_valid {
                        let slice = unsafe { buffer.to_slice_mut::<MaybeUninit<u8>>() };
                        let mut count = 0;
                        for slot in slice {
                            if let Some(item) = mouse.queue.pop() {
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
