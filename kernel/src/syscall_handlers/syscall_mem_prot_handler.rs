use common::{MemProt, SyscallMemProt, SyscallMemProtError};
use itertools::Itertools;
use nodit::{InclusiveInterval, Interval};
use x86_64::VirtAddr;

use crate::{
    EffectiveFlags, GetTableError, MapPageError2, Page, UnmapPageError2, UpdateFlagsError2,
    cpu_local_data::get_local,
    memory::{MEMORY, MemoryType},
    task::{THREADS, UserVirtMem},
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
            let mut process_memory = current_process.memory.write();
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
                let page = Page::new(
                    VirtAddr::new(helper.input().start_page_index.get() as u64),
                    page_size,
                )
                .unwrap()
                .offset(i as u64)
                .unwrap();
                if prot.contains(MemProt::READABLE) {
                    let flags = EffectiveFlags {
                        writable: prot.contains(MemProt::WRITABLE),
                        executable: prot.contains(MemProt::EXECUTABLE),
                        user_accessible: true,
                        global: false,
                    };
                    let result = unsafe { process_memory.l4.update_flags(page, flags) };
                    if let Err(UpdateFlagsError2::GetTable(GetTableError::NotMapped)) = &result {
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
                            process_memory
                                .l4
                                .map_page(page, frame, flags, &mut frame_allocator)
                        };
                        if let Err(MapPageError2::FrameAllocationFailed) = &result {
                            // TODO: Cleanup
                            return Err(SyscallMemProtError::OutOfPhysMem);
                        }
                    } else {
                        result.unwrap();
                    }
                } else {
                    let result = unsafe { process_memory.l4.unmap_page(page) };
                    if !matches!(
                        result,
                        Err(UnmapPageError2::GetTable(GetTableError::NotMapped))
                    ) {
                        result.unwrap();
                    }
                }
            }

            Ok(())
        })();
        helper.syscall_return(&result)
    }
}
