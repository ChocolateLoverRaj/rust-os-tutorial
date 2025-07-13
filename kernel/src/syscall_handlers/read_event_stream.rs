use core::mem::MaybeUninit;

use common::{SyscallReadEventStream, SyscallReadEventStreamInput};
use nodit::Interval;

use crate::{
    cpu_local_data::get_local,
    task::{PS2_EVENT_STREAMS, THREADS},
};

use super::GenericSyscallHandler;

pub struct SyscallReadEventStreamHandler;
impl GenericSyscallHandler for SyscallReadEventStreamHandler {
    type S = SyscallReadEventStream;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        unimplemented!()
        // enum Action {
        //     Terminate,
        //     Return(u64),
        // }
        // let action = {
        //     let SyscallReadEventStreamInput { stream_id, buffer } = helper.input();
        //     let range = buffer.pointer()..=buffer.pointer().saturating_add(buffer.len() - 1);
        //     let threads = THREADS.read();
        //     let local = get_local();
        //     let current_process = &threads
        //         .get(&local.running_thread.lock().unwrap())
        //         .unwrap()
        //         .process;
        //     let event_streams = PS2_EVENT_STREAMS.read();
        //     if let Some(event_stream) = event_streams.get(stream_id) {
        //         if event_stream.process == current_process.id {
        //             let is_valid = current_process
        //                 .memory
        //                 .read()
        //                 .mapped_virtual_memory
        //                 .overlapping(Interval::from(range))
        //                 .all(|(_interval, permissions)| permissions.write);
        //             if is_valid {
        //                 let slice = unsafe { buffer.to_slice_mut::<MaybeUninit<u8>>() };
        //                 let mut count = 0;
        //                 for slot in slice {
        //                     if let Some(item) = event_stream.queue.pop() {
        //                         slot.write(item);
        //                         count += 1;
        //                     } else {
        //                         break;
        //                     }
        //                 }
        //                 Action::Return(count)
        //             } else {
        //                 Action::Terminate
        //             }
        //         } else {
        //             Action::Terminate
        //         }
        //     } else {
        //         Action::Terminate
        //     }
        // };
        // match action {
        //     Action::Return(value) => helper.syscall_return(&value),
        //     Action::Terminate => todo!(),
        // }
    }
}
