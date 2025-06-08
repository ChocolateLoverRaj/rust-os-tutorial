use common::{SyscallAlloc, SyscallAllocError};
use nodit::Interval;
use x86_64::{
    VirtAddr,
    structures::paging::{Mapper, Page, PageSize, PageTableFlags, Size4KiB, mapper::MapToError},
};

use crate::{
    get_page_table::get_page_table,
    memory::{MEMORY, MemoryType, UserModeMemoryUsageType},
    run_user_mode_program::TASK,
};

use super::GenericSyscallHandler;

pub struct SyscallAllocHandler;
impl GenericSyscallHandler for SyscallAllocHandler {
    type S = SyscallAlloc;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Return(Result<(), SyscallAllocError>),
            Terminate,
        }
        let action = {
            let input = helper.input();
            let mut task = TASK.lock();
            let task = task.as_mut().unwrap();
            let range = input.start..=input.start.saturating_add(input.len - 1);
            log::debug!("{range:X?} {:#X?}", task.mapped_virtual_memory);
            let overlaps = task
                .mapped_virtual_memory
                .overlaps(Interval::from(range.clone()));
            if input.start.is_multiple_of(Size4KiB::SIZE)
                && input.len.is_multiple_of(Size4KiB::SIZE)
                && !overlaps
            {
                Action::Return((|| {
                    let memory = MEMORY.get().unwrap();
                    let mut physical_memory = memory.physical_memory.lock();
                    let page_count = input.len / Size4KiB::SIZE;
                    let start_page =
                        Page::<Size4KiB>::from_start_address(VirtAddr::new(input.start)).unwrap();
                    // Safety: we are the only code modifying the task's page tables rn
                    let mut mapper = unsafe { get_page_table(task.cr3, false) };
                    for i in 0..page_count {
                        let frame = physical_memory
                            .allocate_frame_with_type::<Size4KiB>(MemoryType::UsedByUserMode(
                                UserModeMemoryUsageType::Heap,
                            ))
                            .ok_or(SyscallAllocError::OutOfMemory)?;
                        let page = start_page + i;
                        let flags = PageTableFlags::PRESENT
                            | PageTableFlags::USER_ACCESSIBLE
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE;
                        let frame_allocator =
                            &mut physical_memory.get_user_mode_program_frame_allocator();
                        unsafe { mapper.map_to(page, frame, flags, frame_allocator) }
                            .map_err(|e| match e {
                                MapToError::FrameAllocationFailed => SyscallAllocError::OutOfMemory,
                                e => unreachable!("{:#?}", e),
                            })?
                            .flush();
                    }
                    task.mapped_virtual_memory
                        .insert_merge_touching(range.into())
                        .unwrap();
                    Ok(())
                })())
            } else {
                Action::Terminate
            }
        };
        match action {
            Action::Return(r) => helper.syscall_return(&r),
            Action::Terminate => todo!("{:X?}", helper.input()),
        }
    }
}
