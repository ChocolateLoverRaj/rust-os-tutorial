use core::sync::atomic::Ordering;

use common::SyscallSubscribeToKeyboard;
use crossbeam_queue::ArrayQueue;

use crate::task::{EVENT_ID, EventStream, EventStreamSource, TASK};

use super::GenericSyscallHandler;

pub struct SyscallSubscribeToKeyboardHandler;
impl GenericSyscallHandler for SyscallSubscribeToKeyboardHandler {
    type S = SyscallSubscribeToKeyboard;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = {
            let mut task = TASK.try_lock().unwrap();
            let task = task.as_mut().unwrap();
            let event_stream_id = EVENT_ID.fetch_add(1, Ordering::Relaxed);
            task.event_streams.insert(
                event_stream_id,
                EventStream {
                    source: EventStreamSource::Ps2Keyboard,
                    queue: ArrayQueue::new(64),
                    pending_event: false,
                },
            );
            event_stream_id
        };
        helper.syscall_return(&output)
    }
}
