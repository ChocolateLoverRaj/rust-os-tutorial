use core::num::NonZero;

use common::{LOWER_HALF_END, PermissionFlags, SliceData, SyscallMapSharedMem};
use nodit::{InclusiveInterval, Interval};
use x86_64::{
    VirtAddr,
    structures::paging::{Mapper, Page, PageTableFlags, Size4KiB},
};

use crate::{
    VirtMemPermissions,
    capabilities::{CAPABILITIES, CapabilityType},
    cpu_local_data::get_local,
    get_page_table::get_page_table,
    memory::MEMORY,
    shared_mem::SHARED_MEM,
    task::{SharedVirtMem, THREADS, UserVirtMem},
};

use super::GenericSyscallHandler;

pub struct SyscallMapSharedMemHandler;

impl GenericSyscallHandler for SyscallMapSharedMemHandler {
    type S = SyscallMapSharedMem;

    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = {
            let capability_id = helper.input().capability;
            let capabilities = CAPABILITIES.read();
            // FIXME: Don't panic
            let capability = capabilities.get(&capability_id).unwrap();

            let thread_id = get_local().running_thread.try_lock().unwrap().unwrap();
            let threads = THREADS.read();
            let thread = threads.get(&thread_id).unwrap();

            let shared_mem_id = if capability.process_id == thread.process.id.into()
                && let CapabilityType::SharedMem(shared_mem_id) = capability._type
            {
                shared_mem_id
            } else {
                todo!()
            };
            let shared_mem = SHARED_MEM.read();
            let shared_mem = shared_mem.get(&shared_mem_id).unwrap();
            let shared_mem_len = shared_mem.size.byte_len() * shared_mem.phys_frames.len();

            let mut process_mem = thread.process.memory.write();
            let interval = process_mem
                .mapped_virtual_memory
                .gaps_trimmed(Interval::from(NonZero::<u64>::MIN.get()..LOWER_HALF_END))
                .find_map(|interval| {
                    let aligned_start = interval
                        .start()
                        .next_multiple_of(shared_mem.size.size_bytes());
                    let aligned_interval =
                        Interval::from(aligned_start..aligned_start + shared_mem_len as u64);
                    if interval.contains_interval(&aligned_interval) {
                        Some(aligned_interval)
                    } else {
                        None
                    }
                })
                // FIXME: Don't panic
                .unwrap();
            let permissions = VirtMemPermissions::from(PermissionFlags::from_bits_retain(
                helper.input().permission_flags,
            ));
            process_mem
                .mapped_virtual_memory
                .insert_merge_touching_if_values_equal(
                    interval,
                    UserVirtMem::Shared(SharedVirtMem {
                        shared_mem_id,
                        permissions,
                    }),
                )
                .unwrap();

            let l4_frame = thread.process.cr3;
            let mut mapper = unsafe { get_page_table(l4_frame, false) };
            let start_page =
                Page::<Size4KiB>::from_start_address(VirtAddr::new(interval.start())).unwrap();
            let mut phys_mem = MEMORY.get().unwrap().physical_memory.lock();
            let mut frame_allocator =
                phys_mem.get_user_mode_program_frame_allocator(thread.process.id);
            for (i, frame) in shared_mem.phys_frames.iter().enumerate() {
                let page = start_page + i as u64;
                let flags = PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | permissions.page_table_flags();
                // FIXME: Handle out of mem errors
                unsafe { mapper.map_to(page, *frame, flags, &mut frame_allocator) }
                    .unwrap()
                    .flush();
            }

            Ok(SliceData::new(interval.start(), shared_mem_len as u64))
        };
        helper.syscall_return(&output)
    }
}
