use common::SyscallSubscribeToMouse;

use super::GenericSyscallHandler;

pub struct SyscallSubscribeToMouseHandler;
impl GenericSyscallHandler for SyscallSubscribeToMouseHandler {
    type S = SyscallSubscribeToMouse;
    fn handle_decoded_syscall(_helper: super::SyscallHelper<Self::S>) -> ! {
        unimplemented!()
        // let output = {
        //     if init_ps2_mouse::mouse_exists() {
        //         let event_stream_id = EVENT_ID.fetch_add(1, Ordering::Relaxed);
        //         let mut event_streams = PS2_EVENT_STREAMS.write();
        //         let threads = THREADS.read();
        //         let local = get_local();
        //         let current_process = &threads
        //             .get(&local.running_thread.lock().unwrap())
        //             .unwrap()
        //             .process;
        //         event_streams.insert(
        //             event_stream_id,
        //             EventStream {
        //                 process: current_process.id,
        //                 source: EventStreamSource::Ps2Mouse,
        //                 queue: ArrayQueue::new(64),
        //             },
        //         );
        //         Ok(event_stream_id)
        //     } else {
        //         Err(SyscallSubscribeToMouseError::NoPs2Mouse)
        //     }
        // };
        // helper.syscall_return(&output)
    }
}
