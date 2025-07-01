use alloc::format;
use common::{SliceData, SyscallMapModule, SyscallMapModuleError};
use nodit::interval::ee;
use x86_64::{
    structures::paging::{mapper::MapToError, Mapper, Page, PageSize, PageTableFlags, Size4KiB},
    VirtAddr,
};

use crate::{
    cpu_local_data::get_local,
    get_page_table::get_page_table,
    limine_requests::MODULE_REQUEST,
    memory::{MemoryType, MEMORY},
    task::{VirtualMemoryPermissions, THREADS},
    translate_addr::GetFrameSlice,
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
            let module_ptr = module.addr();
            // Safety: module slice is valid and only immutably accessed
            let module_slice = unsafe { core::slice::from_raw_parts(module_ptr, len as usize) };
            let n_pages = len.div_ceil(Size4KiB::SIZE);
            let threads = THREADS.read();
            let local = get_local();
            let current_process = &threads
                .get(&local.running_thread.lock().unwrap())
                .unwrap()
                .process;
            let mut process_memory = current_process.memory.write();
            let range = process_memory
                .mapped_virtual_memory
                .gaps_trimmed(ee(0, 0xffff800000000000))
                .find_map(|gap| {
                    let aligned_start = gap.start().checked_next_multiple_of(Size4KiB::SIZE)?;
                    let required_end_inclusive = aligned_start + (n_pages * Size4KiB::SIZE - 1);
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
                    VirtualMemoryPermissions {
                        read: true,
                        write: false,
                        execute: false,
                    },
                )
                .unwrap();
            let mut mapper = unsafe { get_page_table(current_process.cr3, false) };
            let start_page =
                Page::<Size4KiB>::from_start_address(VirtAddr::new(*range.start())).unwrap();
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            for i in 0..n_pages {
                let frame = physical_memory
                    .allocate_frame_with_type(MemoryType::UsedByUserMode(current_process.id))
                    .ok_or(SyscallMapModuleError::OutOfPhysicalMemory)?;
                // Safety: we have an exclusive reference
                let frame_slice = unsafe { frame.get_slice_mut() };
                let copy_start = (i * Size4KiB::SIZE) as usize;
                let bytes_to_copy = (module_slice.len() - copy_start).min(Size4KiB::SIZE as usize);
                frame_slice[..bytes_to_copy]
                    .copy_from_slice(&module_slice[copy_start..copy_start + bytes_to_copy]);
                // In the last frame, zero unused bytes
                frame_slice[bytes_to_copy..].fill(0);
                let flags = PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::NO_EXECUTE;
                let page = start_page + i;
                let frame_allocator =
                    &mut physical_memory.get_user_mode_program_frame_allocator(current_process.id);
                unsafe { mapper.map_to(page, frame, flags, frame_allocator) }
                    .map_err(|e| match e {
                        MapToError::FrameAllocationFailed => {
                            SyscallMapModuleError::OutOfPhysicalMemory
                        }
                        e => unreachable!("{:#?}", e),
                    })?
                    .flush();
            }
            Ok(SliceData::new(range.start().clone(), len))
        })();
        helper.syscall_return(&output)
    }
}
