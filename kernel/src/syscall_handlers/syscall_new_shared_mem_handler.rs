use common::{SyscallNewShardMemError, SyscallNewSharedMem};
use nodit::NoditSet;

use crate::{
    capabilities::{CAPABILITIES, Capability, CapabilityId, CapabilityType},
    cpu_local_data::get_local,
    memory::{MEMORY, MemoryType},
    shared_mem::{NEXT_SHARED_MEM_ID, SHARED_MEM, SharedMem},
    task::THREADS,
};

use super::GenericSyscallHandler;

pub struct SyscallNewSharedMemHandler;

impl GenericSyscallHandler for SyscallNewSharedMemHandler {
    type S = SyscallNewSharedMem;

    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let mut phys_mem = MEMORY.get().unwrap().physical_memory.lock();
            let shared_mem_id =
                NEXT_SHARED_MEM_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

            let mut shared_mem = SHARED_MEM.write();
            let page_size = helper.input().page_size;
            shared_mem.insert(
                shared_mem_id,
                SharedMem {
                    page_size,
                    phys_mem: {
                        let mut used = NoditSet::default();
                        for _ in 0..helper.input().pages_len.get() {
                            let mem_type = MemoryType::Shared(shared_mem_id);
                            if let Some(phys_frame) =
                                phys_mem.allocate_frame_with_type_2(page_size, mem_type)
                            {
                                used.insert_merge_touching(
                                    {
                                        let start = phys_frame.as_u64();
                                        start..start + page_size.byte_len_u64()
                                    }
                                    .into(),
                                )
                                .unwrap();
                            } else {
                                phys_mem.remove(&mem_type);
                                return Err(SyscallNewShardMemError::OutOfMem);
                            }
                        }
                        used
                    },
                },
            );
            let capability_id = CapabilityId::new_unique();
            let mut capabilities = CAPABILITIES.write();
            let threads = THREADS.read();
            let thread_id = get_local().running_thread.try_lock().unwrap().unwrap();
            let thread = threads.get(&thread_id).unwrap();
            capabilities.insert(
                capability_id.into(),
                Capability {
                    _type: CapabilityType::SharedMem(shared_mem_id),
                    process_id: thread.process.id.into(),
                },
            );
            Ok(capability_id.into())
        })();
        helper.syscall_return(&output);
    }
}
