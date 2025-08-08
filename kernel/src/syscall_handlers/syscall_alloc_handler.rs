use core::num::NonZero;

use common::{LOWER_HALF_END, MemProt, PageSize, SyscallAlloc, SyscallAllocError};
use nodit::interval::ee;
use raw_cpuid::CpuId;
use x86_64::VirtAddr;

use crate::{
    EffectiveFlags, MapPageError2, Page,
    cpu_local_data::get_local,
    memory::{MEMORY, MemoryType},
    task::{THREADS, UserVirtMem},
    translate_addr::TranslateFrame2,
};

use super::GenericSyscallHandler;

pub struct SyscallAllocHandler;
impl GenericSyscallHandler for SyscallAllocHandler {
    type S = SyscallAlloc;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let page_size = match helper.input().page_size {
                PageSize::_4KiB => Ok(PageSize::_4KiB),
                PageSize::_2MiB => Ok(PageSize::_2MiB),
                PageSize::_1GiB => {
                    if CpuId::new()
                        .get_extended_processor_and_feature_identifiers()
                        .is_some_and(|features| features.has_1gib_pages())
                    {
                        Ok(PageSize::_1GiB)
                    } else {
                        Err(SyscallAllocError::PageSizeNotSupported)
                    }
                }
            }?;
            let flags = MemProt::from_bits_retain(helper.input().mem_prot);
            let threads = THREADS.read();
            let local = get_local();
            let current_process = &threads
                .get(&local.running_thread.lock().unwrap())
                .unwrap()
                .process;
            let mut process_memory = current_process.memory.write();

            // Reserve virt mem
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
                        Some(aligned_start..=required_end_inclusive)
                    } else {
                        None
                    }
                })
                .ok_or(SyscallAllocError::OutOfVirtualMemory)?;
            process_memory
                .mapped_virtual_memory
                .insert_merge_touching_if_values_equal(range.clone().into(), UserVirtMem::Plain)
                .unwrap();

            // Map if needed
            if flags.contains(MemProt::READABLE) {
                let start_page = Page::new(VirtAddr::new(*range.start()), page_size).unwrap();
                let memory = MEMORY.get().unwrap();
                let mut physical_memory = memory.physical_memory.lock();
                for i in 0..helper.input().pages_len.get() as u64 {
                    let page = start_page.offset(i).unwrap();
                    let frame = physical_memory
                        .allocate_frame_with_type(
                            helper.input().page_size,
                            MemoryType::UsedByUserMode(current_process.id),
                        )
                        .ok_or(SyscallAllocError::OutOfPhysicalMemory)?;
                    // We could potentially improve performance by not zeroing frames and instead reusing frames released by the same process
                    unsafe {
                        frame
                            .to_page()
                            .start_addr()
                            .as_mut_ptr::<u8>()
                            .write_bytes(0, helper.input().page_size.byte_len())
                    };
                    let flags = EffectiveFlags {
                        writable: true,
                        executable: false,
                        global: false,
                        user_accessible: true,
                    };
                    let frame_allocator = &mut physical_memory
                        .get_user_mode_program_frame_allocator(current_process.id);
                    unsafe {
                        process_memory
                            .l4
                            .map_page(page, frame, flags, frame_allocator)
                    }
                    .map_err(|e| match e {
                        MapPageError2::FrameAllocationFailed => {
                            SyscallAllocError::OutOfPhysicalMemory
                        }
                        e => unreachable!("{:#?}", e),
                    })?;
                }
            }
            Ok(NonZero::new(*range.start() as usize).unwrap())
        })();
        helper.syscall_return(&output)
    }
}
