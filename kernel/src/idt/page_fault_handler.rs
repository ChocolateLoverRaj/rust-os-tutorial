use thiserror::Error;
use x86_64::{
    PrivilegeLevel, VirtAddr,
    registers::control::Cr2,
    structures::{
        idt::{InterruptStackFrame, PageFaultErrorCode},
        paging::{Mapper, Page, PageSize, PageTableFlags, Size2MiB, mapper::MapToError},
    },
};

use crate::{
    get_page_table::get_page_table,
    hlt_loop::hlt_loop,
    memory::{MEMORY, MemoryType, UserModeMemoryUsageType},
    run_user_mode_program::{INITIAL_RSP, TASK},
};

type ExtendBy = Size2MiB;
pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    // Our kernel needs to gracefully handle user mode causing page faults.
    // We should not panic because of anything user mode does.
    if stack_frame.code_segment.rpl() == PrivilegeLevel::Ring3 {
        let mut task = TASK.lock();
        let task = task.as_mut().unwrap();
        let accessed_address = Cr2::read().unwrap();
        // We handle a page fault as a stack overflow if the accessed address was the guard page (within 1 page of the stack bottom)
        if let Some(guard_page) =
            (INITIAL_RSP - task.stack_size)
                .checked_sub(1)
                .map(|addr_in_guard_page| {
                    Page::<ExtendBy>::containing_address(VirtAddr::new(addr_in_guard_page))
                })
            && Page::containing_address(accessed_address) == guard_page
        {
            log::debug!("User mode program did a stack overflow. Extending the stack.");
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            #[derive(Debug, Error)]
            enum ExtendStackError {
                #[error("Failed to allocate frame")]
                OutOfMemory,
            }
            match (|| {
                let mut o = unsafe { get_page_table(task.cr3, false) };
                let frame = physical_memory
                    .allocate_frame_with_type(MemoryType::UsedByUserMode(
                        UserModeMemoryUsageType::Stack,
                    ))
                    .ok_or(ExtendStackError::OutOfMemory)?;
                match unsafe {
                    o.map_to(
                        guard_page,
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::USER_ACCESSIBLE
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE,
                        &mut physical_memory.get_user_mode_program_frame_allocator(),
                    )
                } {
                    Err(MapToError::FrameAllocationFailed) => {
                        Err(ExtendStackError::OutOfMemory)?;
                    }
                    result => result.unwrap().flush(),
                };
                Ok::<_, ExtendStackError>(())
            })() {
                Ok(()) => {
                    task.stack_size += ExtendBy::SIZE;
                }
                Err(e) => {
                    let current_stack_size = task.stack_size;
                    log::warn!(
                        "Error extending stack: {e:?}. Current stack size: 0x{current_stack_size:X?}"
                    );
                    // TODO: Clean up the user mode program
                    hlt_loop()
                }
            }
        } else {
            log::warn!("User mode program did a regular page fault");
            hlt_loop()
        }
    } else {
        let accessed_address = Cr2::read().unwrap();
        panic!(
            "Page fault! Stack frame: {stack_frame:#?}. Error code: {error_code:#?}. Accessed address: {accessed_address:?}."
        )
    }
}
