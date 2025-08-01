use alloc::format;
use common::{HIGHER_HALF_START, SliceData, SyscallMapModule, SyscallMapModuleError};
use nodit::interval::ee;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        Mapper, Page, PageSize, PageTableFlags, PhysFrame, Size4KiB, mapper::MapToError,
    },
};

use crate::{
    cpu_local_data::get_local,
    get_page_table::get_page_table,
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
            let first_frame = PhysFrame::<Size4KiB>::from_start_address(PhysAddr::new(
                module.addr() as u64 - u64::from(HhdmOffset::get_from_response()),
            ))
            .unwrap();
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
                .gaps_trimmed(ee(0, HIGHER_HALF_START))
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
                    UserVirtMem::LimineModule,
                )
                .unwrap();
            let mut mapper = unsafe { get_page_table(current_process.cr3, false) };
            let start_page =
                Page::<Size4KiB>::from_start_address(VirtAddr::new(*range.start())).unwrap();
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            for i in 0..n_pages {
                let flags = PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::NO_EXECUTE;
                let page = start_page + i;
                let frame = first_frame + i;
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
            // Zero unused bytes of last frame
            // If this module gets mapped multiple times, we don't need to zero the bytes the 2nd+ times
            // But to keep things simple we can just zero every time
            let bytes_to_copy = (n_pages * Size4KiB::SIZE - len) as usize;
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
