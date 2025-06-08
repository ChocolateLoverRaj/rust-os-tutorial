use common::{
    SyscallReleaseFrameBuffer, SyscallTakeFrameBuffer, SyscallTakeFrameBufferError,
    SyscallTakeFrameBufferOutput,
};
use nodit::interval::ue;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        Mapper, Page, PageSize, PageTableFlags, PhysFrame, Size4KiB, mapper::MapToError,
    },
};

use crate::{
    get_page_table::get_page_table, hhdm_offset::HhdmOffset, limine_requests::FRAME_BUFFER_REQUEST,
    logger, memory::MEMORY, run_user_mode_program::TASK,
};

use super::GenericSyscallHandler;

pub struct SyscallTakeFrameBufferHandler;
impl GenericSyscallHandler for SyscallTakeFrameBufferHandler {
    type S = SyscallTakeFrameBuffer;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        helper.syscall_return(&(|| {
            let frame_buffer = FRAME_BUFFER_REQUEST
                .get_response()
                .unwrap()
                .framebuffers()
                .next()
                .ok_or(SyscallTakeFrameBufferError::NotAvailable)?;
            let frame_buffer_len = frame_buffer.pitch() * frame_buffer.height();
            if !((frame_buffer.addr() as u64).is_multiple_of(Size4KiB::SIZE)
                && frame_buffer_len.is_multiple_of(Size4KiB::SIZE))
            {
                Err(SyscallTakeFrameBufferError::WouldNotBeSecure)?;
            }
            logger::take_frame_buffer().ok_or(SyscallTakeFrameBufferError::InUse)?;
            let mut task = TASK.lock();
            let task = task.as_mut().unwrap();
            let range = task
                .mapped_virtual_memory
                .gaps_trimmed(ue(0xffff800000000000))
                .find_map(|range| {
                    let aligned_start = range.start().next_multiple_of(Size4KiB::SIZE);
                    let needed_end_inclusive = aligned_start + (frame_buffer_len - 1);
                    if needed_end_inclusive <= range.end() {
                        Some(aligned_start..=needed_end_inclusive)
                    } else {
                        None
                    }
                })
                .ok_or(SyscallTakeFrameBufferError::OutOfVirtualMemory)?;
            let first_frame = PhysFrame::<Size4KiB>::from_start_address(PhysAddr::new(
                frame_buffer.addr() as u64 - u64::from(HhdmOffset::get_from_response()),
            ))
            .unwrap();
            let first_page = Page::from_start_address(VirtAddr::new(*range.start())).unwrap();
            let n_pages = frame_buffer_len / Size4KiB::SIZE;
            // Zero the frame buffer to not leak data
            // Safety: we are only accessing frame buffer memory
            unsafe {
                frame_buffer
                    .addr()
                    .write_bytes(0, frame_buffer_len as usize)
            };
            // Safety: the page table is valid
            let mut mapper = unsafe { get_page_table(task.cr3, false) };
            let mut physical_memory = MEMORY.get().unwrap().physical_memory.lock();
            for i in 0..n_pages {
                let frame = first_frame + i;
                let page = first_page + i;
                let flags = PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::NO_EXECUTE;
                let frame_allocator = &mut physical_memory.get_user_mode_program_frame_allocator();
                // Safety: virtual memory is unused, physical memory is okay to access
                unsafe { mapper.map_to(page, frame, flags, frame_allocator) }
                    .map_err(|e| match e {
                        MapToError::FrameAllocationFailed => {
                            SyscallTakeFrameBufferError::OutOfPhysicalMemory
                        }
                        e => unreachable!("{:#?}", e),
                    })?
                    .flush();
            }
            Ok(SyscallTakeFrameBufferOutput {
                ptr: *range.start(),
                info: (&frame_buffer).into(),
            })
        })())
    }
}

pub struct SyscallReleaseFrameBufferHandler;
impl GenericSyscallHandler for SyscallReleaseFrameBufferHandler {
    type S = SyscallReleaseFrameBuffer;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        // TODO: Actually release the frame buffer
        helper.syscall_return(&())
    }
}
