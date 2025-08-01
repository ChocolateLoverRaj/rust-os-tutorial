use common::{AllocPageSize, SyscallNewSharedMem};
use nodit::{Interval, NoditSet};
use x86_64::structures::paging::{PageSize, Size1GiB, Size2MiB, Size4KiB};

use crate::{
    capabilities::{CAPABILITIES, Capability, CapabilityId, CapabilityType},
    cpu_local_data::get_local,
    memory::{MEMORY, MemoryType, PhysicalMemory},
    shared_mem::{NEXT_SHARED_MEM_ID, SHARED_MEM, SharedMem},
    task::THREADS,
};

use super::GenericSyscallHandler;

pub struct SyscallNewSharedMemHandler;

impl GenericSyscallHandler for SyscallNewSharedMemHandler {
    type S = SyscallNewSharedMem;

    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = {
            let mut phys_mem = MEMORY.get().unwrap().physical_memory.lock();
            let shared_mem_id =
                NEXT_SHARED_MEM_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            fn allocate_shared_mem<T: PageSize>(
                pages_len: usize,
                phys_mem: &mut PhysicalMemory,
                shared_mem_id: u64,
            ) -> NoditSet<u64, Interval<u64>> {
                let mut used = NoditSet::default();
                for _ in 0..pages_len {
                    if let Some(phys_frame) =
                        phys_mem.allocate_frame_with_type::<T>(MemoryType::Shared(shared_mem_id))
                    {
                        used.insert_merge_touching(
                            {
                                let start = phys_frame.start_address().as_u64();
                                start..start + phys_frame.size()
                            }
                            .into(),
                        )
                        .unwrap();
                    } else {
                        todo!("Clean up frames and return err")
                    }
                }
                used
            }
            let mut shared_mem = SHARED_MEM.write();
            let page_size = helper.input().page_size;
            shared_mem.insert(
                shared_mem_id,
                SharedMem {
                    page_size,
                    phys_mem: match page_size {
                        AllocPageSize::_4KiB => allocate_shared_mem::<Size4KiB>(
                            helper.input().pages_len,
                            &mut phys_mem,
                            shared_mem_id,
                        ),
                        AllocPageSize::_2MiB => allocate_shared_mem::<Size2MiB>(
                            helper.input().pages_len,
                            &mut phys_mem,
                            shared_mem_id,
                        ),
                        AllocPageSize::_1GiB => allocate_shared_mem::<Size1GiB>(
                            helper.input().pages_len,
                            &mut phys_mem,
                            shared_mem_id,
                        ),
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
        };
        helper.syscall_return(&output);
    }
}
