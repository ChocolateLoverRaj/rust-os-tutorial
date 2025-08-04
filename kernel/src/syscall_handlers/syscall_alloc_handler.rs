use core::num::NonZero;

use alloc::collections::btree_set::BTreeSet;
use common::{AllocPageSize, LOWER_HALF_END, SyscallAlloc, SyscallAllocError};
use nodit::interval::ee;
use raw_cpuid::CpuId;
use x86_64::{VirtAddr, structures::paging::PageTableFlags};

use crate::{
    MapPageError,
    cpu_local_data::get_local,
    map_page,
    memory::{MEMORY, MemoryType},
    task::{THREADS, UserVirtMem},
    translate_addr::TranslateAddr,
};

use super::GenericSyscallHandler;

pub struct SyscallAllocHandler;
impl GenericSyscallHandler for SyscallAllocHandler {
    type S = SyscallAlloc;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let page_size = match helper.input().page_size {
                AllocPageSize::_4KiB => Ok(AllocPageSize::_4KiB),
                AllocPageSize::_2MiB => Ok(AllocPageSize::_2MiB),
                AllocPageSize::_1GiB => {
                    if {
                        CpuId::new()
                            .get_extended_processor_and_feature_identifiers()
                            .is_some_and(|features| features.has_1gib_pages())
                    } {
                        Ok(AllocPageSize::_1GiB)
                    } else {
                        Err(SyscallAllocError::PageSizeNotSupported)
                    }
                }
            }?;
            let threads = THREADS.read();
            let local = get_local();
            let current_process = &threads
                .get(&local.running_thread.lock().unwrap())
                .unwrap()
                .process;
            let mut process_memory = current_process.memory.write();
            let range = process_memory
                .mapped_virtual_memory
                .gaps_trimmed(ee(0, LOWER_HALF_END))
                .find_map(|gap| {
                    let aligned_start = gap
                        .start()
                        .checked_next_multiple_of(page_size.byte_len_u64())?;
                    let required_end_inclusive = aligned_start
                        + helper.input().pages_len.get() as u64
                            * helper.input().page_size.byte_len_u64()
                        - 1;
                    if required_end_inclusive <= gap.end() {
                        Some(aligned_start..required_end_inclusive)
                    } else {
                        None
                    }
                })
                .ok_or(SyscallAllocError::OutOfVirtualMemory)?;
            process_memory
                .mapped_virtual_memory
                .insert_merge_touching_if_values_equal(range.clone().into(), UserVirtMem::Plain)
                .unwrap();
            let start_page =
                VirtAddr::new(range.start).align_down(helper.input().page_size.byte_len_u64());
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            for i in 0..helper.input().pages_len.get() as u64 {
                let page = start_page + i * helper.input().page_size.byte_len_u64();
                let frame = physical_memory
                    .allocate_frame_with_type_2(
                        helper.input().page_size,
                        MemoryType::UsedByUserMode(BTreeSet::from([current_process.id])),
                    )
                    .ok_or(SyscallAllocError::OutOfPhysicalMemory)?;
                // We could potentially improve performance by not zeroing frames and instead reusing frames released by the same process
                unsafe {
                    frame
                        .to_virt()
                        .as_mut_ptr::<u8>()
                        .write_bytes(0, helper.input().page_size.byte_len())
                };
                let flags = PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::NO_EXECUTE;
                let frame_allocator =
                    &mut physical_memory.get_user_mode_program_frame_allocator(current_process.id);
                unsafe {
                    map_page(
                        current_process.cr3,
                        helper.input().page_size,
                        page,
                        frame,
                        flags,
                        frame_allocator,
                    )
                }
                .map_err(|e| match e {
                    MapPageError::AllocateFrame => SyscallAllocError::OutOfPhysicalMemory,
                    e => unreachable!("{:#?}", e),
                })?;
            }
            // log::debug!("Allocated for user mode: {range:X?}");
            Ok(NonZero::new(range.start as usize).unwrap())
        })();
        helper.syscall_return(&output)
    }
}
