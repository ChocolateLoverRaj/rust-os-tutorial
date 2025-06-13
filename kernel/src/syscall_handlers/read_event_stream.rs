use core::mem::MaybeUninit;

use common::{SyscallReadEventStream, SyscallReadEventStreamInput};
use nodit::Interval;

use crate::task::TASK;

use super::GenericSyscallHandler;

pub struct SyscallReadEventStreamHandler;
impl GenericSyscallHandler for SyscallReadEventStreamHandler {
    type S = SyscallReadEventStream;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Terminate,
            Return(u64),
        }
        let action = {
            let SyscallReadEventStreamInput { stream_id, buffer } = helper.input();
            let range = buffer.pointer()..=buffer.pointer().saturating_add(buffer.len() - 1);
            let mut task = TASK.try_lock().unwrap();
            let task = task.as_mut().unwrap();
            if let Some(event_stream) = task.event_streams.get(stream_id) {
                let is_valid = task
                    .mapped_virtual_memory
                    .overlapping(Interval::from(range))
                    .all(|(_interval, permissions)| permissions.write);
                if is_valid {
                    let slice = unsafe { buffer.to_slice_mut::<MaybeUninit<u8>>() };
                    let mut count = 0;
                    for slot in slice {
                        if let Some(item) = event_stream.queue.pop() {
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
        };
        match action {
            Action::Return(value) => helper.syscall_return(&value),
            Action::Terminate => todo!(),
        }
    }
}
