use common::SyscallShareCapability;

use crate::{
    capabilities::{CAPABILITIES, Capability, CapabilityId},
    cpu_local_data::get_local,
    task::THREADS,
};

use super::GenericSyscallHandler;

pub struct SyscallShareCapabilityHandler;

impl GenericSyscallHandler for SyscallShareCapabilityHandler {
    type S = SyscallShareCapability;

    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = {
            let capability_id = helper.input().capability;
            let mut capabilities = CAPABILITIES.write();
            // FIXME: Don't panic if the capability is not found
            let capability = capabilities.get(&capability_id.into()).unwrap();
            let thread_id = get_local().running_thread.try_lock().unwrap().unwrap();
            let threads = THREADS.read();
            let thread = threads.get(&thread_id).unwrap();
            if capability.process_id != thread.process.id.into() {
                todo!()
            }
            let new_capability_id = CapabilityId::new_unique();
            let _type = capability._type.clone();
            capabilities.insert(
                new_capability_id,
                Capability {
                    _type,
                    process_id: helper.input().process_id,
                },
            );
            new_capability_id.into()
        };
        helper.syscall_return(&output)
    }
}
