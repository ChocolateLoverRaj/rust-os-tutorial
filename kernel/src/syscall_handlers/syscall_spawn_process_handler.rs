use core::{num::NonZero, ops::Deref};

use alloc::{boxed::Box, collections::btree_set::BTreeSet, sync::Arc};
use common::{
    LOWER_HALF_END, SpawnProcessMemoryFlags, SpawnProcessMemoryMapping,
    SpawnProcessRelativePriority, Syscall, SyscallSpawnProcess, SyscallSpawnProcessError,
    SyscallSpawnProcessInput,
};
use nodit::{Interval, NoditMap};
use x2apic::lapic::IpiAllShorthand;
use x86_64::{
    VirtAddr,
    structures::paging::{PageTableFlags, Size4KiB},
};
use zerocopy::TryFromBytes;

use crate::{
    CAPABILITIES,
    cpu_local_data::get_local,
    get_page_table::get_page_table,
    interrupt_vector::InterruptVector,
    map_page,
    memory::{MEMORY, MemoryType},
    run_tasks::run_threads,
    task::{
        Process, ProcessId, ProcessMemory, StartData, THREAD_PRIORITIES, THREADS, Thread, ThreadId,
        ThreadReadyState, ThreadReadyStateInSyscall, ThreadState, UserVirtMem,
    },
    try_access_user_mem::try_access_user_mem,
    unmap_page,
};

use super::GenericSyscallHandler;

pub struct SyscallSpawnProcessHandler;
impl GenericSyscallHandler for SyscallSpawnProcessHandler {
    type S = SyscallSpawnProcess;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let result = (|| {
            let input_ptr = *helper.input();
            if input_ptr == 0
                || input_ptr + u64::try_from(size_of::<SyscallSpawnProcessInput>()).unwrap()
                    > LOWER_HALF_END
            {
                Err(SyscallSpawnProcessError::InvalidInputPtr)?
            }
            let input_ptr = input_ptr as *const [u8; size_of::<SyscallSpawnProcessInput>()];
            let input = try_access_user_mem(|| Box::new(unsafe { input_ptr.read() }))
                .map_err(|_e| SyscallSpawnProcessError::InvalidInputPtr)?;
            let input = SyscallSpawnProcessInput::try_ref_from_bytes(input.deref())
                .map_err(|_| SyscallSpawnProcessError::InvalidInput)?;
            if input.memory_mappings.pointer() == 0
                || input.memory_mappings.pointer()
                    + input.memory_mappings.len()
                        * u64::try_from(size_of::<SpawnProcessMemoryMapping>()).unwrap()
                    > LOWER_HALF_END
            {
                Err(SyscallSpawnProcessError::InvalidInputPtr)?
            }
            if input.send_capabilities.pointer() == 0
                || input.send_capabilities.pointer()
                    + input.send_capabilities.len()
                        * u64::try_from(size_of::<NonZero<u64>>()).unwrap()
                    > LOWER_HALF_END
            {
                Err(SyscallSpawnProcessError::InvalidInputPtr)?
            }

            let local = get_local();
            let mut running_thread_lock = local.running_thread.try_lock().unwrap();
            let running_thread_id = running_thread_lock.unwrap();
            let mut threads = THREADS.write();
            let running_thread = threads.get(&running_thread_id).unwrap();

            // FIXME: Clean up if errors are found
            let new_process_id = ProcessId::new_unique();
            let mut capabilities = CAPABILITIES.write();
            for i in 0..input.send_capabilities.len() as usize {
                let capability = try_access_user_mem(|| {
                    let capability_ptr = (input.send_capabilities.pointer() as usize
                        + i * size_of::<u64>())
                        as *const u64;
                    Box::new(unsafe { capability_ptr.read() })
                })
                .map_err(|_e| SyscallSpawnProcessError::InvalidCapabilityPtr)?;
                let capability_id = NonZero::new(*capability)
                    .ok_or(SyscallSpawnProcessError::InvalidCapabilityId)?;
                let capability = capabilities
                    .get_mut(&capability_id)
                    .ok_or(SyscallSpawnProcessError::CapabilityNotFound)?;
                if capability.process_id != running_thread.process.id.into() {
                    Err(SyscallSpawnProcessError::CapabilityNotFound)?
                }
                capability.process_id = new_process_id.into();
            }

            let mut process_memory = running_thread.process.memory.write();
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            let new_cr3 = physical_memory
                .allocate_frame_with_type::<Size4KiB>(MemoryType::UsedByUserMode(BTreeSet::from([
                    new_process_id,
                ])))
                .ok_or(SyscallSpawnProcessError::OutOfPhysMem)?;
            let mut new_virt_mem = NoditMap::default();
            let current_mapper = unsafe { get_page_table(running_thread.process.cr3, false) };
            let mut new_mapper = unsafe { get_page_table(new_cr3, true) };
            for i in 256..512 {
                new_mapper.level_4_table_mut()[i].clone_from(&current_mapper.level_4_table()[i]);
            }

            for i in 0..input.memory_mappings.len() as usize {
                let memory_mapping_ptr = (input.memory_mappings.pointer() as usize
                    + i * size_of::<SpawnProcessMemoryMapping>())
                    as *const [u8; size_of::<SpawnProcessMemoryMapping>()];
                let memory_mapping =
                    try_access_user_mem(|| Box::new(unsafe { memory_mapping_ptr.read() }))
                        .map_err(|_e| SyscallSpawnProcessError::InvalidMemoryMappingPtr)?;
                let memory_mapping =
                    SpawnProcessMemoryMapping::try_ref_from_bytes(memory_mapping.deref())
                        .map_err(|_e| SyscallSpawnProcessError::InvalidMemoryMapping)?;
                let memory_mapping_flags =
                    SpawnProcessMemoryFlags::from_bits_retain(memory_mapping.flags);
                let page_size = memory_mapping_flags.page_size();
                let _ = process_memory.mapped_virtual_memory.cut({
                    let start = u64::try_from(memory_mapping.current_process_start).unwrap();
                    Interval::from(
                        start
                            ..start
                                .checked_add(
                                    u64::try_from(memory_mapping.pages_len).unwrap()
                                        * page_size.byte_len_u64(),
                                )
                                .ok_or(SyscallSpawnProcessError::InvalidMemoryMappingSrc)?,
                    )
                });
                new_virt_mem
                    .insert_merge_touching_if_values_equal(
                        {
                            let start = u64::try_from(memory_mapping.new_process_start).unwrap();
                            Interval::from(
                                start
                                    ..start
                                        .checked_add(
                                            u64::try_from(memory_mapping.pages_len).unwrap()
                                                * page_size.byte_len_u64(),
                                        )
                                        .ok_or(SyscallSpawnProcessError::InvalidMemoryMappingSrc)?,
                            )
                        },
                        UserVirtMem::Plain,
                    )
                    .unwrap();
                let start_page_current = VirtAddr::new(memory_mapping.current_process_start as u64);
                let start_page_new = VirtAddr::new(memory_mapping.new_process_start as u64);
                for i in 0..memory_mapping.pages_len {
                    let page = start_page_current + i as u64 * page_size.byte_len_u64();
                    let frame = unsafe { unmap_page(running_thread.process.cr3, page_size, page) }
                        .unwrap()
                        .addr();
                    physical_memory.share_memory(page_size, frame, new_process_id);
                    physical_memory.unshare_memory(page_size, frame, running_thread.process.id);
                    let page = start_page_new + i as u64 * page_size.byte_len_u64();
                    let flags = PageTableFlags::PRESENT
                        | PageTableFlags::USER_ACCESSIBLE
                        | memory_mapping_flags.into();
                    let mut frame_allocator =
                        physical_memory.get_user_mode_program_frame_allocator(new_process_id);
                    // FIXME: Gracefully handle out of memory
                    unsafe {
                        map_page(new_cr3, page_size, page, frame, flags, &mut frame_allocator)
                    }
                    .unwrap();
                }
            }
            let new_thread_id = ThreadId::new_unique();
            drop(process_memory);
            *running_thread_lock = None;
            *running_thread.state.write() =
                ThreadState::Ready(ThreadReadyState::InSyscall(ThreadReadyStateInSyscall {
                    saved_regs: helper.saved_regs().clone(),
                    output: <Self::S as Syscall>::encode_output(&Ok(new_process_id.into())),
                }));
            threads.insert(
                new_thread_id,
                Thread {
                    process: Arc::new(Process {
                        cr3: new_cr3,
                        id: new_process_id,
                        memory: spin::RwLock::new(ProcessMemory {
                            mapped_virtual_memory: new_virt_mem,
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
            let new_thread_position = match input.priority {
                SpawnProcessRelativePriority::Lower => running_thread_position + 1,
                SpawnProcessRelativePriority::Higher => running_thread_position,
            };
            thread_priorities.insert(new_thread_position, new_thread_id);
            let mut local_apic = local.local_apic.get().unwrap().try_lock().unwrap();
            unsafe {
                local_apic.send_ipi_all(
                    InterruptVector::CheckTasks.into(),
                    IpiAllShorthand::AllExcludingSelf,
                );
            }
            Ok(())
        })();
        match result {
            Err(e) => helper.syscall_return(&Err(e)),
            Ok(_) => run_threads(),
        }
    }
}
