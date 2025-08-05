use core::num::NonZero;

use common::{
    LOWER_HALF_END, PermissionFlags, SliceData, SyscallMapSharedMem, SyscallMapSharedMemError,
};
use nodit::{InclusiveInterval, Interval};
use x86_64::{PhysAddr, VirtAddr, structures::paging::PageTableFlags};

use crate::{
    capabilities::{CAPABILITIES, CapabilityType},
    cpu_local_data::get_local,
    map_page,
    memory::MEMORY,
    shared_mem::SHARED_MEM,
    task::{SharedVirtMem, THREADS, UserVirtMem},
};

use super::GenericSyscallHandler;

pub struct SyscallMapSharedMemHandler;

impl GenericSyscallHandler for SyscallMapSharedMemHandler {
    type S = SyscallMapSharedMem;

    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let capability_id = helper.input().capability;
            let capabilities = CAPABILITIES.read();
            let capability = capabilities
                .get(&capability_id)
                .ok_or(SyscallMapSharedMemError::CapabilityNotFound)?;

            let thread_id = get_local().running_thread.try_lock().unwrap().unwrap();
            let threads = THREADS.read();
            let thread = threads.get(&thread_id).unwrap();

            if capability.process_id != thread.process.id.into() {
                Err(SyscallMapSharedMemError::CapabilityNotFound)?
            }
            let shared_mem_id = if let CapabilityType::SharedMem(shared_mem_id) = capability._type {
                shared_mem_id
            } else {
                Err(SyscallMapSharedMemError::InvalidCapability)?
            };
            let shared_mem = SHARED_MEM.read();
            let shared_mem = shared_mem.get(&shared_mem_id).unwrap();
            let shared_mem_len = shared_mem
                .phys_mem
                .iter()
                .map(|interval| interval.end() - interval.start() + 1)
                .sum::<u64>();

            let mut process_mem = thread.process.memory.write();
            let interval = process_mem
                .mapped_virtual_memory
                .gaps_trimmed(Interval::from(NonZero::<u64>::MIN.get()..LOWER_HALF_END))
                .find_map(|interval| {
                    let aligned_start = interval
                        .start()
                        .next_multiple_of(shared_mem.page_size.byte_len_u64());
                    let aligned_interval =
                        Interval::from(aligned_start..aligned_start + shared_mem_len);
                    if interval.contains_interval(&aligned_interval) {
                        Some(aligned_interval)
                    } else {
                        None
                    }
                })
                .ok_or(SyscallMapSharedMemError::NoVirtMem)?;
            process_mem
                .mapped_virtual_memory
                .insert_merge_touching_if_values_equal(
                    interval,
                    UserVirtMem::Shared(SharedVirtMem { shared_mem_id }),
                )
                .unwrap();

            let mut phys_mem = MEMORY.get().unwrap().physical_memory.lock();
            let mut frame_allocator =
                phys_mem.get_user_mode_program_frame_allocator(thread.process.id);
            let flags = PageTableFlags::PRESENT
                | PageTableFlags::USER_ACCESSIBLE
                | PermissionFlags::from_bits_retain(helper.input().permission_flags)
                    .page_table_flags();
            let start_page = VirtAddr::new(interval.start());
            let mut pages_mapped = 0;
            for interval in shared_mem.phys_mem.iter() {
                let start_frame = PhysAddr::new(interval.start());
                let frames_len =
                    (interval.end() - interval.start() + 1) / shared_mem.page_size.byte_len_u64();
                for i in 0..frames_len {
                    let page = start_page + pages_mapped * shared_mem.page_size.byte_len_u64();
                    let frame = start_frame + i * shared_mem.page_size.byte_len_u64();
                    log::trace!("Mapping {page:?} to {frame:?}");
                    unsafe {
                        map_page(
                            thread.process.cr3,
                            shared_mem.page_size,
                            page,
                            frame,
                            flags,
                            &mut frame_allocator,
                        )
                    }
                    .unwrap();
                    // FIXME: Handle out of mem errors
                    pages_mapped += 1;
                }
            }

            Ok(SliceData::new(interval.start(), shared_mem_len))
        })();
        helper.syscall_return(&output)
    }
}
