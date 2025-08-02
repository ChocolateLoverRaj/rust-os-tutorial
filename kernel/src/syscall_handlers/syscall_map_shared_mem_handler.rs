use core::{fmt::Debug, num::NonZero};

use common::{
    AllocPageSize, LOWER_HALF_END, PermissionFlags, SliceData, SyscallMapSharedMem,
    SyscallMapSharedMemError,
};
use nodit::{InclusiveInterval, Interval};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        Mapper, Page, PageSize, PageTableFlags, PhysFrame, Size1GiB, Size2MiB, Size4KiB,
    },
};

use crate::{
    capabilities::{CAPABILITIES, CapabilityType},
    cpu_local_data::get_local,
    get_page_table::get_page_table,
    memory::{MEMORY, PhysicalMemoryFrameAllocator},
    shared_mem::{SHARED_MEM, SharedMem},
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
            // FIXME: Don't panic
            let capability = capabilities.get(&capability_id).unwrap();

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
            let shared_mem_len = shared_mem.page_size.byte_len() * shared_mem.phys_mem.len();

            let mut process_mem = thread.process.memory.write();
            let interval = process_mem
                .mapped_virtual_memory
                .gaps_trimmed(Interval::from(NonZero::<u64>::MIN.get()..LOWER_HALF_END))
                .find_map(|interval| {
                    let aligned_start = interval
                        .start()
                        .next_multiple_of(shared_mem.page_size.byte_len_u64());
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
            process_mem
                .mapped_virtual_memory
                .insert_merge_touching_if_values_equal(
                    interval,
                    UserVirtMem::Shared(SharedVirtMem { shared_mem_id }),
                )
                .unwrap();

            let l4_frame = thread.process.cr3;
            let mut mapper = unsafe { get_page_table(l4_frame, false) };

            let mut phys_mem = MEMORY.get().unwrap().physical_memory.lock();
            let mut frame_allocator =
                phys_mem.get_user_mode_program_frame_allocator(thread.process.id);
            let flags = PageTableFlags::PRESENT
                | PageTableFlags::USER_ACCESSIBLE
                | PermissionFlags::from_bits_retain(helper.input().permission_flags)
                    .page_table_flags();
            pub fn map<T: PageSize + Debug, M: Mapper<T>>(
                shared_mem: &SharedMem,
                flags: PageTableFlags,
                frame_allocator: &mut PhysicalMemoryFrameAllocator,
                mapper: &mut M,
                start_addr: VirtAddr,
            ) {
                let start_page = Page::<T>::from_start_address(start_addr).unwrap();
                let mut pages_mapped = 0;
                for interval in shared_mem.phys_mem.iter() {
                    let start_frame =
                        PhysFrame::<T>::from_start_address(PhysAddr::new(interval.start()))
                            .unwrap();
                    let frames_len = (interval.end() - interval.start() + 1) / T::SIZE;
                    for i in 0..frames_len {
                        let page = start_page + pages_mapped;
                        let frame = start_frame + i;
                        // FIXME: Handle out of mem errors
                        unsafe { mapper.map_to(page, frame, flags, frame_allocator) }
                            .unwrap()
                            .flush();
                        pages_mapped += 1;
                    }
                }
            }
            let start_addr = VirtAddr::new(interval.start());
            match shared_mem.page_size {
                AllocPageSize::_4KiB => map::<Size4KiB, _>(
                    shared_mem,
                    flags,
                    &mut frame_allocator,
                    &mut mapper,
                    start_addr,
                ),
                AllocPageSize::_2MiB => map::<Size2MiB, _>(
                    shared_mem,
                    flags,
                    &mut frame_allocator,
                    &mut mapper,
                    start_addr,
                ),
                AllocPageSize::_1GiB => map::<Size1GiB, _>(
                    shared_mem,
                    flags,
                    &mut frame_allocator,
                    &mut mapper,
                    start_addr,
                ),
            }

            Ok(SliceData::new(interval.start(), shared_mem_len as u64))
        })();
        helper.syscall_return(&output)
    }
}
