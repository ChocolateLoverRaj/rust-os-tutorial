use common::{MemProt, SyscallMemProt, SyscallMemProtError};
use itertools::Itertools;
use nodit::{InclusiveInterval, Interval};
use x86_64::{VirtAddr, structures::paging::PageTableFlags};

use crate::{
    MapPageError, UnmapPageError, UpdateFlagsError,
    cpu_local_data::get_local,
    map_page,
    memory::{MEMORY, MemoryType},
    task::{THREADS, UserVirtMem},
    unmap_page, update_flags,
};

use super::GenericSyscallHandler;

pub struct SyscallMemProtHandler;
impl GenericSyscallHandler for SyscallMemProtHandler {
    type S = SyscallMemProt;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let result = (|| {
            let threads = THREADS.read();
            let local = get_local();
            let current_process = &threads
                .get(&local.running_thread.lock().unwrap())
                .unwrap()
                .process;
            // We still have to obtain a write lock to safely map pages
            let process_memory = current_process.memory.write();
            // Make sure the mem is plain
            let page_size = helper.input().page_size;
            let interval = Interval::from({
                let start = helper
                    .input()
                    .start_page_index
                    .get()
                    .checked_mul(page_size.byte_len())
                    .ok_or(SyscallMemProtError::InvalidInterval)?
                    as u64;
                start
                    ..start
                        .checked_add(
                            helper
                                .input()
                                .pages_len
                                .get()
                                .checked_mul(page_size.byte_len())
                                .ok_or(SyscallMemProtError::InvalidInterval)?
                                as u64,
                        )
                        .ok_or(SyscallMemProtError::InvalidInterval)?
            });
            let (overlapping_interval, mem) = process_memory
                .mapped_virtual_memory
                .overlapping(interval)
                .exactly_one()
                .map_err(|_| SyscallMemProtError::NotPlain)?;
            if overlapping_interval.contains_interval(&interval) {
                return Err(SyscallMemProtError::NotPlain);
            }
            if !matches!(mem, UserVirtMem::Plain) {
                return Err(SyscallMemProtError::NotPlain);
            }
            let prot = MemProt::from_bits_retain(helper.input().new_prot);

            // Actually change mappings
            for i in 0..helper.input().pages_len.get() {
                let page = VirtAddr::new(
                    ((helper.input().start_page_index.get() + i) * page_size.byte_len()) as u64,
                );
                if prot.contains(MemProt::READABLE) {
                    let flags = {
                        let mut flags = PageTableFlags::USER_ACCESSIBLE;
                        if prot.contains(MemProt::WRITABLE) {
                            flags |= PageTableFlags::WRITABLE;
                        }
                        if !prot.contains(MemProt::EXECUTABLE) {
                            flags |= PageTableFlags::NO_EXECUTE;
                        }
                        flags
                    };
                    let result =
                        unsafe { update_flags(current_process.cr3, page_size, page, flags) };
                    if let Err(UpdateFlagsError::NotMapped) = &result {
                        let mut phys_mem = MEMORY.get().unwrap().physical_memory.lock();
                        let frame = if let Some(frame) = phys_mem.allocate_frame_with_type(
                            page_size,
                            MemoryType::UsedByUserMode(current_process.id),
                        ) {
                            frame
                        } else {
                            // TODO: Maybe cleanup
                            return Err(SyscallMemProtError::OutOfPhysMem);
                        };
                        let mut frame_allocator =
                            phys_mem.get_user_mode_program_frame_allocator(current_process.id);
                        let result = unsafe {
                            map_page(
                                current_process.cr3,
                                page_size,
                                page,
                                frame,
                                flags,
                                &mut frame_allocator,
                            )
                        };
                        if let Err(MapPageError::AllocateFrame) = &result {
                            // TODO: Cleanup
                            return Err(SyscallMemProtError::OutOfPhysMem);
                        }
                    } else {
                        result.unwrap();
                    }
                } else {
                    let result = unsafe { unmap_page(current_process.cr3, page_size, page) };
                    if !matches!(result, Err(UnmapPageError::NotMapped)) {
                        result.unwrap();
                    }
                }
            }

            Ok(())
        })();
        helper.syscall_return(&result)
    }
}
