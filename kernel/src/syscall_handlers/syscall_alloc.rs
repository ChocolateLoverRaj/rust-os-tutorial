use core::fmt::Debug;

use common::{SliceData, SyscallAlloc, SyscallAllocError};
use nodit::interval::ue;
use x86_64::{
    VirtAddr,
    structures::paging::{
        Mapper, OffsetPageTable, Page, PageSize, PageTableFlags, PhysFrame, Size2MiB, Size4KiB,
        mapper::MapToError,
    },
};

use crate::{
    get_page_table::get_page_table,
    memory::{MEMORY, MemoryType, UserModeMemoryUsageType},
    run_user_mode_program::TASK,
    translate_addr::ZeroFrame,
};

use super::GenericSyscallHandler;

pub struct SyscallAllocHandler;
impl GenericSyscallHandler for SyscallAllocHandler {
    type S = SyscallAlloc;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let align = u64::from(helper.input().align);
        if align.checked_next_power_of_two() == Some(align) {
            helper.syscall_return(&(|| {
                let len = u64::from(helper.input().len);
                fn map_with_page_size<S: PageSize + Debug>(
                    len: u64,
                    align: u64,
                ) -> Result<SliceData, SyscallAllocError>
                where
                    for<'a> OffsetPageTable<'a>: Mapper<S>,
                    PhysFrame<S>: ZeroFrame,
                {
                    let n_pages = len.div_ceil(S::SIZE);
                    let mut task = TASK.lock();
                    let task = task.as_mut().unwrap();
                    let range = task
                        .mapped_virtual_memory
                        .gaps_trimmed(ue(0xffff800000000000))
                        .find_map(|gap| {
                            let aligned_start = gap
                                .start()
                                .max(1)
                                .checked_next_multiple_of(S::SIZE.max(align))?;
                            let required_end_inclusive = aligned_start + (n_pages * S::SIZE - 1);
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
                        Page::<S>::from_start_address(VirtAddr::new(*range.start())).unwrap();
                    let end_page_inclusive =
                        Page::<S>::containing_address(VirtAddr::new(*range.end()));
                    let memory = MEMORY.get().unwrap();
                    let mut physical_memory = memory.physical_memory.lock();
                    for page in start_page..=end_page_inclusive {
                        let frame = physical_memory
                            .allocate_frame_with_type(MemoryType::UsedByUserMode(
                                UserModeMemoryUsageType::Heap,
                            ))
                            .ok_or(SyscallAllocError::OutOfPhysicalMemory)?;
                        unsafe { frame.zero() }
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
                    Ok(SliceData::new(*range.start(), n_pages * S::SIZE))
                }
                if len < Size2MiB::SIZE {
                    map_with_page_size::<Size4KiB>(len, align)
                } else {
                    map_with_page_size::<Size2MiB>(len, align)
                }
            })())
        } else {
            todo!()
        }
    }
}
