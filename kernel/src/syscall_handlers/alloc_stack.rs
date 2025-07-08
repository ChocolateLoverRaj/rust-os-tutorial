use alloc::collections::btree_set::BTreeSet;
use common::{SliceData, SyscallAllocStack, SyscallAllocStackError, SyscallAllocStackOutput};
use nodit::interval::ue;
use x86_64::{
    VirtAddr,
    structures::paging::{Mapper, Page, PageSize, PageTableFlags, Size4KiB, mapper::MapToError},
};

use crate::{
    cpu_local_data::get_local,
    get_page_table::get_page_table,
    memory::{MEMORY, MemoryType},
    task::{THREADS, VirtualMemoryPermissions},
    translate_addr::ZeroFrame,
};

use super::GenericSyscallHandler;

pub struct SyscallAllocStackHandler;
impl GenericSyscallHandler for SyscallAllocStackHandler {
    type S = SyscallAllocStack;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let result = (|| {
            let len = helper.input().len;
            let n_pages = len.div_ceil(Size4KiB::SIZE) + 1;
            let threads = THREADS.read();
            let local = get_local();
            let current_process = &threads
                .get(&local.running_thread.lock().unwrap())
                .unwrap()
                .process;
            let mut process_memory = current_process.memory.write();
            let range = process_memory
                .mapped_virtual_memory
                .gaps_trimmed(ue(0xffff800000000000))
                .find_map(|gap| {
                    let aligned_start = gap.start().checked_next_multiple_of(Size4KiB::SIZE)?;
                    let required_end_inclusive = aligned_start + (n_pages * Size4KiB::SIZE - 1);
                    if required_end_inclusive <= gap.end() {
                        Some(aligned_start..=required_end_inclusive)
                    } else {
                        None
                    }
                })
                .ok_or(SyscallAllocStackError::OutOfVirtualMemory)?;
            let guard_page_range = *range.start()..=*range.start() + (Size4KiB::SIZE - 1);
            process_memory
                .mapped_virtual_memory
                .insert_merge_touching_if_values_equal(
                    guard_page_range.into(),
                    VirtualMemoryPermissions {
                        read: false,
                        write: false,
                        execute: false,
                    },
                )
                .unwrap();
            let usable_range = *range.start() + Size4KiB::SIZE..=*range.end();
            process_memory
                .mapped_virtual_memory
                .insert_merge_touching_if_values_equal(
                    usable_range.clone().into(),
                    VirtualMemoryPermissions {
                        read: true,
                        write: true,
                        execute: false,
                    },
                )
                .unwrap();
            let mut mapper = unsafe { get_page_table(current_process.cr3, false) };
            let start_page =
                Page::<Size4KiB>::from_start_address(VirtAddr::new(*usable_range.start())).unwrap();
            let end_page_inclusive =
                Page::<Size4KiB>::containing_address(VirtAddr::new(*usable_range.end()));
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            for page in start_page..=end_page_inclusive {
                let frame = physical_memory
                    .allocate_frame_with_type(MemoryType::UsedByUserMode(BTreeSet::from([
                        current_process.id,
                    ])))
                    .ok_or(SyscallAllocStackError::OutOfPhysicalMemory)?;
                unsafe { frame.zero() }
                let flags = PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::NO_EXECUTE;
                let frame_allocator =
                    &mut physical_memory.get_user_mode_program_frame_allocator(current_process.id);
                unsafe { mapper.map_to(page, frame, flags, frame_allocator) }
                    .map_err(|e| match e {
                        MapToError::FrameAllocationFailed => {
                            SyscallAllocStackError::OutOfPhysicalMemory
                        }
                        e => unreachable!("{:#?}", e),
                    })?
                    .flush();
            }
            Ok(SyscallAllocStackOutput {
                usable_stack: SliceData::new(*usable_range.start(), (n_pages - 1) * Size4KiB::SIZE),
            })
        })();
        helper.syscall_return(&result)
    }
}
