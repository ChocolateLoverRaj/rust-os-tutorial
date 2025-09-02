use alloc::format;
use common::{LOWER_HALF_END, PageSize, SliceData, SyscallMapModule, SyscallMapModuleError};
use ez_paging::{ConfigurableFlags, Frame, MapPageError, Page};
use nodit::interval::ee;
use x86_64::{PhysAddr, VirtAddr, registers::model_specific::PatMemoryType};

use crate::{
    cpu_local_data::get_local,
    hhdm_offset::HhdmOffset,
    limine_requests::MODULE_REQUEST,
    memory::MEMORY,
    task::{THREADS, UserVirtMem},
};

use super::GenericSyscallHandler;

pub struct SyscallMapModuleHandler;
impl GenericSyscallHandler for SyscallMapModuleHandler {
    type S = SyscallMapModule;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = (|| {
            let index = helper.input();
            let module = MODULE_REQUEST
                .get_response()
                .unwrap()
                .modules()
                .iter()
                .find(|file| file.path().to_str() == Ok(&format!("/extra_module_{index}")))
                .ok_or(SyscallMapModuleError::NotPresent)?;
            let len = module.size();
            let page_size = PageSize::_4KiB;
            let first_frame = Frame::new(
                PhysAddr::new(module.addr() as u64 - u64::from(HhdmOffset::get_from_response())),
                page_size,
            )
            .unwrap();
            let n_pages = len.div_ceil(page_size.byte_len_u64());
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
                    let required_end_inclusive =
                        aligned_start + (n_pages * page_size.byte_len_u64() - 1);
                    if required_end_inclusive <= gap.end() {
                        Some(aligned_start..=required_end_inclusive)
                    } else {
                        None
                    }
                })
                .ok_or(SyscallMapModuleError::OutOfVirtualMemory)?;
            process_memory
                .mapped_virtual_memory
                .insert_merge_touching_if_values_equal(
                    range.clone().into(),
                    UserVirtMem::LimineModule,
                )
                .unwrap();
            let start_page = Page::new(VirtAddr::new(*range.start()), page_size).unwrap();
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            for i in 0..n_pages {
                let page = start_page.offset(i).unwrap();
                let frame = first_frame.offset(i).unwrap();
                let flags = ConfigurableFlags {
                    writable: false,
                    executable: false,
                    pat_memory_type: PatMemoryType::WriteBack,
                };
                let frame_allocator =
                    &mut physical_memory.get_user_mode_program_frame_allocator(current_process.id);
                log::trace!("Mapping {page:?}->{frame:?}");
                unsafe {
                    process_memory
                        .l4
                        .map_page(page, frame, flags, frame_allocator)
                }
                .map_err(|e| match e {
                    MapPageError::FrameAllocationFailed => {
                        SyscallMapModuleError::OutOfPhysicalMemory
                    }
                    e => unreachable!("{:#?}", e),
                })?;
            }
            // Zero unused bytes of last frame
            // If this module gets mapped multiple times, we don't need to zero the bytes the 2nd+ times
            // But to keep things simple we can just zero every time
            let bytes_to_copy = (n_pages * page_size.byte_len_u64() - len) as usize;
            unsafe {
                module
                    .addr()
                    .add(len as usize)
                    .write_bytes(Default::default(), bytes_to_copy);
            }
            Ok(SliceData::new(*range.start(), len))
        })();
        helper.syscall_return(&output)
    }
}
