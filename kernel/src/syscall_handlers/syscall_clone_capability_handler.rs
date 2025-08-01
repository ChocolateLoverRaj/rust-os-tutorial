use common::{SyscallCloneCapability, SyscallCloneCapabilityError};

use crate::{
    capabilities::{CAPABILITIES, Capability, CapabilityId},
    cpu_local_data::get_local,
    task::THREADS,
};

use super::GenericSyscallHandler;

pub struct SyscallCloneCapabilityHandler;

impl GenericSyscallHandler for SyscallCloneCapabilityHandler {
    type S = SyscallCloneCapability;

    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let capability_id = *helper.input();
            let mut capabilities = CAPABILITIES.write();
            let capability = capabilities
                .get(&capability_id)
                .ok_or(SyscallCloneCapabilityError::InvalidCapability)?;
            let thread_id = get_local().running_thread.try_lock().unwrap().unwrap();
            let threads = THREADS.read();
            let thread = threads.get(&thread_id).unwrap();
            if capability.process_id != thread.process.id.into() {
                Err(SyscallCloneCapabilityError::InvalidCapability)?
            }
            let _type = capability
                ._type
                .try_clone()
                .ok_or(SyscallCloneCapabilityError::CannotClone)?;
            let new_capability_id = CapabilityId::new_unique();
            capabilities.insert(
                new_capability_id.into(),
                Capability {
                    _type,
                    process_id: thread.process.id.into(),
                },
            );
            Ok(new_capability_id.into())
        })();
        helper.syscall_return(&output)
    }
}
