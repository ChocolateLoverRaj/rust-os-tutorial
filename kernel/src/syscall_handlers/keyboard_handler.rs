use core::sync::atomic::{AtomicU8, Ordering};

use common::{SyscallSubscribeToKeyboard, SyscallSubscribeToKeyboardError};

use crate::{
    cpu_local_data::get_local,
    event_stream_mem::EventStreamMem,
    task::{
        CAPABILITIES, CapabilityType, EVENT_ID, EventStream, EventStreamSource, PS2_EVENT_STREAMS,
        THREADS,
    },
};

use super::GenericSyscallHandler;

pub struct SyscallSubscribeToKeyboardHandler;
impl GenericSyscallHandler for SyscallSubscribeToKeyboardHandler {
    type S = SyscallSubscribeToKeyboard;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let mut event_streams = PS2_EVENT_STREAMS.write();
            let threads = THREADS.read();
            let local = get_local();
            let current_process = &threads
                .get(&local.running_thread.lock().unwrap())
                .unwrap()
                .process;

            // Check permissions
            let capabilities = CAPABILITIES.read();
            let capability = capabilities
                .get(&helper.input().capability.into())
                .ok_or(SyscallSubscribeToKeyboardError::InvalidCapability)?;
            if !(capability._type == CapabilityType::Ps2Keyboard
                && capability.process.id == current_process.id)
            {
                Err(SyscallSubscribeToKeyboardError::InvalidCapability)?;
            }

            let mem_ptr = helper.input().queue_ptr as *mut EventStreamMem;
            let lower_half_end = 0x800000000000;
            if !(mem_ptr.is_aligned()
                && mem_ptr.addr() <= lower_half_end
                && mem_ptr.addr() + size_of::<EventStreamMem>() <= lower_half_end)
            {
                todo!()
            }
            let mem = unsafe { mem_ptr.as_mut() }.unwrap();
            let slots_len = mem.slots_len;
            let slots_ptr = mem_ptr.addr() + size_of::<EventStreamMem>();
            let slots_ptr_end = slots_ptr + size_of::<AtomicU8>() * slots_len;
            if !(slots_ptr_end <= lower_half_end) {
                todo!()
            }
            let event_stream_id = EVENT_ID.fetch_add(1, Ordering::Relaxed);
            event_streams.insert(
                event_stream_id,
                EventStream {
                    process: current_process.clone(),
                    source: EventStreamSource::Ps2Keyboard,
                    ptr: mem_ptr.addr(),
                },
            );
            Ok(event_stream_id)
        })();
        helper.syscall_return(&output)
    }
}
