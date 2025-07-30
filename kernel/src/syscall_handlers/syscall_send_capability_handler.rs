use common::{SyscallSendCapability, SyscallSendCapabilityError};

use crate::{capabilities::CAPABILITIES, cpu_local_data::get_local, task::THREADS};

use super::GenericSyscallHandler;

pub struct SyscallSendCapabilityHandler;

impl GenericSyscallHandler for SyscallSendCapabilityHandler {
    type S = SyscallSendCapability;

    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let capability_id = helper.input().capability;
            let mut capabilities = CAPABILITIES.write();
            let capability = capabilities
                .get_mut(&capability_id)
                .ok_or(SyscallSendCapabilityError::InvalidCapability)?;
            let thread_id = get_local().running_thread.try_lock().unwrap().unwrap();
            let threads = THREADS.read();
            let thread = threads.get(&thread_id).unwrap();
            if capability.process_id != thread.process.id.into() {
                Err(SyscallSendCapabilityError::InvalidCapability)?
            }
            if !capability._type.can_send() {
                Err(SyscallSendCapabilityError::CannotSend)?
            }
            capability.process_id = helper.input().process_id;
            Ok(())
        })();
        helper.syscall_return(&output)
    }
}
