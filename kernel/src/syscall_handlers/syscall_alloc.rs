use common::{SliceData, SyscallAlloc, SyscallAllocError};
use nodit::interval::ue;
use x86_64::{
    VirtAddr,
    structures::paging::{Mapper, Page, PageSize, PageTableFlags, Size4KiB, mapper::MapToError},
};

use crate::{
    get_page_table::get_page_table,
    memory::{MEMORY, MemoryType, UserModeMemoryUsageType},
    run_user_mode_program::TASK,
    translate_addr::GetFrameSlice,
};

use super::GenericSyscallHandler;

pub struct SyscallAllocHandler;
impl GenericSyscallHandler for SyscallAllocHandler {
    type S = SyscallAlloc;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let align = u64::from(helper.input().align);
        if align.checked_next_power_of_two() == Some(align) {
            helper.syscall_return(&(|| {
                let n_pages = u64::from(helper.input().len).div_ceil(Size4KiB::SIZE);
                let mut task = TASK.lock();
                let task = task.as_mut().unwrap();
                let range = task
                    .mapped_virtual_memory
                    .gaps_trimmed(ue(0xffff800000000000))
                    .find_map(|gap| {
                        let aligned_start = gap
                            .start()
                            .max(1)
                            .checked_next_multiple_of(Size4KiB::SIZE.max(align))?;
                        let required_end_inclusive = aligned_start + (n_pages * Size4KiB::SIZE - 1);
                        if required_end_inclusive <= gap.end() {
                            Some(aligned_start..=required_end_inclusive)
                        } else {
                            None
                        }
                    })
                    .ok_or(SyscallAllocError::OutOfVirtualMemory)?;
                task.mapped_virtual_memory
                    .insert_merge_touching(range.clone().into())
                    .unwrap();
                let mut mapper = unsafe { get_page_table(task.cr3, false) };
                let start_page =
                    Page::<Size4KiB>::from_start_address(VirtAddr::new(*range.start())).unwrap();
                let end_page_inclusive =
                    Page::<Size4KiB>::containing_address(VirtAddr::new(*range.end()));
                let memory = MEMORY.get().unwrap();
                let mut physical_memory = memory.physical_memory.lock();
                for page in start_page..=end_page_inclusive {
                    let frame = physical_memory
                        .allocate_frame_with_type(MemoryType::UsedByUserMode(
                            UserModeMemoryUsageType::Heap,
                        ))
                        .ok_or(SyscallAllocError::OutOfPhysicalMemory)?;
                    // Zero the frame
                    unsafe { frame.get_slice_mut() }.fill(Default::default());
                    let flags = PageTableFlags::PRESENT
                        | PageTableFlags::USER_ACCESSIBLE
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::NO_EXECUTE;
                    let frame_allocator =
                        &mut physical_memory.get_user_mode_program_frame_allocator();
                    unsafe { mapper.map_to(page, frame, flags, frame_allocator) }
                        .map_err(|e| match e {
                            MapToError::FrameAllocationFailed => {
                                SyscallAllocError::OutOfPhysicalMemory
                            }
                            e => unreachable!("{:#?}", e),
                        })?
                        .flush();
                }
                Ok(SliceData::new(*range.start(), n_pages * Size4KiB::SIZE))
            })())
        } else {
            todo!()
        }
    }
}
