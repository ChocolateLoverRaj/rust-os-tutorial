use common::{
    HIGHER_HALF_START, PageSize, SyscallReleaseFrameBuffer, SyscallTakeFrameBuffer,
    SyscallTakeFrameBufferError, SyscallTakeFrameBufferOutput,
};
use nodit::interval::ue;
use x86_64::{PhysAddr, VirtAddr, registers::model_specific::PatMemoryType};

use crate::{
    EffectiveFlags, Frame, MapPageError2, Page,
    cpu_local_data::get_local,
    hhdm_offset::HhdmOffset,
    limine_requests::FRAME_BUFFER_REQUEST,
    logger,
    memory::MEMORY,
    task::{THREADS, UserVirtMem},
};

use super::GenericSyscallHandler;

const PAGE_SIZE: PageSize = PageSize::_4KiB;

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
            if !(frame_buffer
                .addr()
                .addr()
                .is_multiple_of(PAGE_SIZE.byte_len())
                && frame_buffer_len.is_multiple_of(PAGE_SIZE.byte_len_u64()))
            {
                Err(SyscallTakeFrameBufferError::WouldNotBeSecure)?;
            }
            logger::take_frame_buffer().ok_or(SyscallTakeFrameBufferError::InUse)?;
            let threads = THREADS.read();
            let local = get_local();
            let current_process = &threads
                .get(&local.running_thread.lock().unwrap())
                .unwrap()
                .process;
            let mut process_memory = current_process.memory.write();
            let range = process_memory
                .mapped_virtual_memory
                .gaps_trimmed(ue(HIGHER_HALF_START))
                .find_map(|range| {
                    let aligned_start = range.start().next_multiple_of(PAGE_SIZE.byte_len_u64());
                    let needed_end_inclusive = aligned_start + (frame_buffer_len - 1);
                    if needed_end_inclusive <= range.end() {
                        Some(aligned_start..=needed_end_inclusive)
                    } else {
                        None
                    }
                })
                .ok_or(SyscallTakeFrameBufferError::OutOfVirtualMemory)?;
            process_memory
                .mapped_virtual_memory
                .insert_merge_touching_if_values_equal(
                    range.clone().into(),
                    UserVirtMem::FrameBuffer,
                )
                .unwrap();
            // process_memory.frame_buffer_virtual_start = Some(*range.start());
            let first_frame = Frame::new(
                PhysAddr::new(
                    frame_buffer.addr() as u64 - u64::from(HhdmOffset::get_from_response()),
                ),
                PAGE_SIZE,
            )
            .unwrap();
            let first_page = Page::new(VirtAddr::new(*range.start()), PAGE_SIZE).unwrap();
            let n_pages = frame_buffer_len / PAGE_SIZE.byte_len_u64();
            // Zero the frame buffer to not leak data
            // Safety: we are only accessing frame buffer memory
            unsafe {
                frame_buffer
                    .addr()
                    .write_bytes(0, frame_buffer_len as usize)
            };
            let mut physical_memory = MEMORY.get().unwrap().physical_memory.lock();
            for i in 0..n_pages {
                let frame = first_frame.offset(i).unwrap();
                let page = first_page.offset(i).unwrap();
                let flags = EffectiveFlags {
                    writable: true,
                    executable: false,
                    global: false,
                    user_accessible: true,
                    pat_memory_type: PatMemoryType::WriteCombining,
                };
                let frame_allocator =
                    &mut physical_memory.get_user_mode_program_frame_allocator(current_process.id);
                // Safety: virtual memory is unused, physical memory is okay to access
                unsafe {
                    process_memory
                        .l4
                        .map_page(page, frame, flags, frame_allocator)
                }
                .map_err(|e| match e {
                    MapPageError2::FrameAllocationFailed => {
                        SyscallTakeFrameBufferError::OutOfPhysicalMemory
                    }
                    e => unreachable!("{:#?}", e),
                })?;
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
        enum Action {
            Terminate,
            Return,
        }
        let action = {
            let local = get_local();
            let threads = THREADS.read();
            let running_thread_id = local.running_thread.try_lock().unwrap().unwrap();
            let runnning_thread = threads.get(&running_thread_id).unwrap();
            let mut process_memory = runnning_thread.process.memory.write();
            let frame_buffer_interval =
                process_memory
                    .mapped_virtual_memory
                    .iter()
                    .find_map(|(interval, mem)| {
                        if let UserVirtMem::FrameBuffer = mem {
                            Some(*interval)
                        } else {
                            None
                        }
                    });
            if let Some(frame_buffer_interval) = frame_buffer_interval {
                let frame_buffer_response = FRAME_BUFFER_REQUEST.get_response().unwrap();
                let frame_buffer = frame_buffer_response.framebuffers().next().unwrap();
                let frame_buffer_len = frame_buffer.pitch() * frame_buffer.height();
                let n_pages = frame_buffer_len / PAGE_SIZE.byte_len_u64();
                let start_page =
                    Page::new(VirtAddr::new(frame_buffer_interval.start()), PAGE_SIZE).unwrap();
                for i in 0..n_pages {
                    let page = start_page.offset(i).unwrap();
                    unsafe { process_memory.l4.unmap_page(page) }.unwrap();
                }
                let _ = process_memory
                    .mapped_virtual_memory
                    .cut(frame_buffer_interval);
                logger::init_frame_buffer(frame_buffer_response, true);
                log::debug!(
                    "User mode program released frame buffer. Frame buffer will again be used by the kernel for logging."
                );
                Action::Return
            } else {
                Action::Terminate
            }
        };
        match action {
            Action::Return => helper.syscall_return(&()),
            Action::Terminate => todo!(),
        }
    }
}
