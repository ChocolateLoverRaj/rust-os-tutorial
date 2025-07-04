use core::sync::atomic::Ordering;

use common::SyscallSubscribeToKeyboard;
use crossbeam_queue::ArrayQueue;

use crate::{
    cpu_local_data::get_local,
    task::{EVENT_ID, EventStream, EventStreamSource, PS2_EVENT_STREAMS, THREADS},
};

use super::GenericSyscallHandler;

pub struct SyscallSubscribeToKeyboardHandler;
impl GenericSyscallHandler for SyscallSubscribeToKeyboardHandler {
    type S = SyscallSubscribeToKeyboard;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = {
            let event_stream_id = EVENT_ID.fetch_add(1, Ordering::Relaxed);
            let mut event_streams = PS2_EVENT_STREAMS.write();
            let threads = THREADS.read();
            let local = get_local();
            let current_process = &threads
                .get(&local.running_thread.lock().unwrap())
                .unwrap()
                .process;
            event_streams.insert(
                event_stream_id,
                EventStream {
                    process: current_process.id,
                    source: EventStreamSource::Ps2Keyboard,
                    queue: ArrayQueue::new(64),
                },
            );
            event_stream_id
        };
        helper.syscall_return(&output)
    }
}
