use alloc::sync::Arc;
use common::{
    PagePermissions, ProcessRelativePriority, SpawnProcessMemoryMapping, SyscallSpawnProcess,
};
use nodit::{Interval, NoditMap};
use x86_64::{
    VirtAddr,
    structures::paging::{Mapper, Page, PageSize, Size4KiB},
};

use crate::{
    cpu_local_data::get_local,
    get_page_table::get_page_table,
    memory::{MEMORY, MemoryType},
    task::{
        Process, ProcessId, ProcessMemory, StartData, THREAD_PRIORITIES, THREADS, Thread, ThreadId,
        ThreadReadyState, ThreadState,
    },
};

use super::GenericSyscallHandler;

pub struct SyscallSpawnProcessHandler;
impl GenericSyscallHandler for SyscallSpawnProcessHandler {
    type S = SyscallSpawnProcess;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        enum Action {
            Terminate,
            Return,
        }
        let result = (|| {
            let input = helper.input();
            let local = get_local();
            let running_thread_id = local.running_thread.try_lock().unwrap().unwrap();
            let mut threads = THREADS.write();
            let running_thread = threads.get(&running_thread_id).unwrap();
            let mut process_memory = running_thread.process.memory.write();

            let interval = Interval::from(
                input.memory_mappings.pointer()
                    ..input
                        .memory_mappings
                        .pointer()
                        .checked_add(input.memory_mappings.len())
                        .ok_or(())?,
            );
            if !(process_memory
                .mapped_virtual_memory
                .contains_interval(interval)
                && process_memory
                    .mapped_virtual_memory
                    .overlapping(interval)
                    .all(|(_, permissions)| permissions.read))
            {
                Err(())?
            }
            let memory_mappings = unsafe {
                input
                    .memory_mappings
                    .try_to_slice::<SpawnProcessMemoryMapping>()
            }
            .ok_or(())?;
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            let new_process_id = ProcessId::new_unique();
            let new_cr3 = physical_memory
                .allocate_frame_with_type::<Size4KiB>(MemoryType::UsedByUserMode(new_process_id))
                .unwrap(); // TODO: Don't panic here
            let mut mapped_virtual_memory = NoditMap::default();
            let mut current_mapper = unsafe { get_page_table(running_thread.process.cr3, false) };
            let mut new_mapper = unsafe { get_page_table(new_cr3, true) };
            for i in 256..512 {
                new_mapper.level_4_table_mut()[i].clone_from(&current_mapper.level_4_table()[i]);
            }
            for memory_mapping in memory_mappings {
                log::debug!("Mapping: {memory_mapping:#X?}");
                // Don't let another thread modify this data while we're using it
                let memory_mapping = memory_mapping.clone();
                let interval = Interval::from(
                    memory_mapping.current_process_start
                        ..memory_mapping
                            .current_process_start
                            .checked_add(memory_mapping.len)
                            .ok_or(())?,
                );
                if !(process_memory
                    .mapped_virtual_memory
                    .contains_interval(interval)
                    && process_memory
                        .mapped_virtual_memory
                        .overlapping(interval)
                        .all(|(_, permissions)| permissions.read)
                    && memory_mapping
                        .current_process_start
                        .is_multiple_of(Size4KiB::SIZE)
                    && memory_mapping.len.is_multiple_of(Size4KiB::SIZE))
                    && memory_mapping
                        .new_process_start
                        .is_multiple_of(Size4KiB::SIZE)
                    && memory_mapping
                        .new_process_start
                        .checked_add(memory_mapping.len)
                        .ok_or(())?
                        <= 0x800000000000
                {
                    Err(())?
                }
                let _ = process_memory.mapped_virtual_memory.cut(interval);
                let interval = Interval::from(
                    memory_mapping.new_process_start
                        ..memory_mapping.new_process_start + memory_mapping.len,
                );
                let page_permissions =
                    PagePermissions::from_bits_retain(memory_mapping.permissions);
                mapped_virtual_memory
                    .insert_merge_touching_if_values_equal(interval, page_permissions.into())
                    .map_err(|_| ())?;
                let start_page_current = Page::<Size4KiB>::from_start_address(VirtAddr::new(
                    memory_mapping.current_process_start,
                ))
                .unwrap();
                let start_page_new = Page::<Size4KiB>::from_start_address(VirtAddr::new(
                    memory_mapping.new_process_start,
                ))
                .unwrap();
                let page_count = memory_mapping.len / Size4KiB::SIZE;
                for i in 0..page_count {
                    let page = start_page_current + i;
                    // log::debug!("Unmapping {page:?}");
                    let (frame, _flags, flush) = current_mapper.unmap(page).unwrap();
                    flush.flush();

                    let page = start_page_new + i;
                    let flags = page_permissions.into();
                    let mut frame_allocator =
                        physical_memory.get_user_mode_program_frame_allocator(new_process_id);
                    // log::debug!(
                    //     "new process: mapping {page:?} to {frame:?} {:?}",
                    //     new_mapper.level_4_table()
                    // );
                    // FIXME: Gracefully handle out of memory
                    unsafe { new_mapper.map_to(page, frame, flags, &mut frame_allocator) }
                        .unwrap()
                        .flush();
                }
            }
            let new_thread_id = ThreadId::new_unique();
            drop(process_memory);
            threads.insert(
                new_thread_id,
                Thread {
                    process: Arc::new(Process {
                        cr3: new_cr3,
                        id: new_process_id,
                        memory: spin::RwLock::new(ProcessMemory {
                            frame_buffer_virtual_start: None,
                            mapped_virtual_memory,
                        }),
                        mutexes: Default::default(),
                    }),
                    state: spin::RwLock::new(ThreadState::Ready(ThreadReadyState::ReadyToStart(
                        StartData {
                            rip: input.rip,
                            rsp: input.rsp,
                        },
                    ))),
                },
            );
            let mut thread_priorities = THREAD_PRIORITIES.write();
            let running_thread_position = thread_priorities
                .iter()
                .position(|thread_id| *thread_id == running_thread_id)
                .unwrap();
            let new_thread_position = match helper.input().priority {
                ProcessRelativePriority::Lower => running_thread_position + 1,
                ProcessRelativePriority::Higher => running_thread_position,
            };
            thread_priorities.insert(new_thread_position, new_thread_id);
            Ok::<_, ()>(())
        })();
        match result {
            Err(_) => todo!(),
            Ok(_) => helper.syscall_return(&()),
        }
    }
}
