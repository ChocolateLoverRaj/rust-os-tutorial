use core::sync::atomic::AtomicU8;

use common::{LOWER_HALF_END, SyscallSubscribeToKeyboard, SyscallSubscribeToKeyboardError};

use crate::{
    Capability, CapabilityId, EventStream, EventStreamSource,
    capabilities::{CAPABILITIES, CapabilityType},
    cpu_local_data::get_local,
    event_stream_mem::EventStreamMem,
    task::THREADS,
};

use super::GenericSyscallHandler;

pub struct SyscallSubscribeToKeyboardHandler;
impl GenericSyscallHandler for SyscallSubscribeToKeyboardHandler {
    type S = SyscallSubscribeToKeyboard;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let mem_ptr = helper.input().queue_ptr;
            let slots_ptr = mem_ptr
                .checked_add(size_of::<EventStreamMem>())
                .ok_or(SyscallSubscribeToKeyboardError::InvalidQueuePtr)?;
            let slots_len = helper.input().slots_len;
            let slots_ptr_end = slots_ptr
                .checked_add(
                    size_of::<AtomicU8>()
                        .checked_mul(slots_len)
                        .ok_or(SyscallSubscribeToKeyboardError::InvalidQueuePtr)?,
                )
                .ok_or(SyscallSubscribeToKeyboardError::InvalidQueuePtr)?;
            if slots_ptr == 0 || slots_ptr_end as u64 > LOWER_HALF_END {
                Err(SyscallSubscribeToKeyboardError::InvalidQueuePtr)?
            }

            let threads = THREADS.read();
            let local = get_local();
            let current_process = &threads
                .get(&local.running_thread.lock().unwrap())
                .unwrap()
                .process;

            // Check permissions
            let mut capabilities = CAPABILITIES.write();
            let capability = capabilities
                .get(&helper.input().capability)
                .ok_or(SyscallSubscribeToKeyboardError::InvalidCapability)?;
            if !(matches!(capability._type, CapabilityType::Ps2Keyboard)
                && capability.process_id == current_process.id.into())
            {
                Err(SyscallSubscribeToKeyboardError::InvalidCapability)?;
            }

            let capability_id = CapabilityId::new_unique();
            capabilities.insert(
                capability_id.into(),
                Capability {
                    _type: CapabilityType::EventStream(EventStream {
                        process: current_process.clone(),
                        source: EventStreamSource::Ps2Keyboard,
                        ptr: mem_ptr,
                        slots_len,
                    }),
                    process_id: current_process.id.into(),
                },
            );
            Ok(capability_id.into())
        })();
        helper.syscall_return(&output)
    }
}
